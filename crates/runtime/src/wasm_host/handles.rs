use crate::{KList, KMap, KNativeFunction, KString, KTuple, KValue, Ptr};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, LazyLock, Weak},
};
use wasmi::{Caller, Extern};

use super::runtime::WasmRuntime;

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct RegisteredFunction {
    pub symbol: String,
    pub user_data: u32,
}

#[derive(Clone)]
pub(super) struct RuntimeRegisteredFunction {
    pub registered: RegisteredFunction,
}

static RUNTIME_WASM_FUNCTIONS: LazyLock<Mutex<HashMap<String, RuntimeRegisteredFunction>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn runtime_function_key(function: &KNativeFunction) -> String {
    Ptr::address(&function.function).to_string()
}

pub(super) fn register_runtime_wasm_function(
    function: &KNativeFunction,
    registered: RegisteredFunction,
) {
    RUNTIME_WASM_FUNCTIONS.lock().insert(
        runtime_function_key(function),
        RuntimeRegisteredFunction { registered },
    );
}

pub(super) fn lookup_runtime_wasm_function(
    function: &KNativeFunction,
) -> Option<RuntimeRegisteredFunction> {
    RUNTIME_WASM_FUNCTIONS
        .lock()
        .get(&runtime_function_key(function))
        .cloned()
}

#[derive(Clone)]
pub(super) enum WasmHandle {
    String(KString),
    List(KList),
    Tuple(KTuple),
    Map(KMap),
    Object(u32),
    Iterator(u32),
    NativeFunction(RegisteredFunction),
    ValueView(KValue),
    MapData(Vec<(KValue, KValue)>),
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) enum GuestResource {
    Object(u32),
    Iterator(u32),
    NativeFunction(u32),
}

impl WasmHandle {
    pub fn guest_resource(&self) -> Option<GuestResource> {
        match self {
            WasmHandle::Object(user_data) => Some(GuestResource::Object(*user_data)),
            WasmHandle::Iterator(user_data) => Some(GuestResource::Iterator(*user_data)),
            WasmHandle::NativeFunction(registered) => {
                Some(GuestResource::NativeFunction(registered.user_data))
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(super) struct HandleSlot {
    pub generation: u16,
    pub value: Option<WasmHandle>,
}

#[derive(Clone, Default)]
pub(super) struct HandleTable {
    slots: Vec<HandleSlot>,
    free: Vec<u16>,
}

impl HandleTable {
    pub fn insert(&mut self, value: WasmHandle) -> u32 {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(slot.value.is_none());
            slot.value = Some(value);
            Self::pack_handle(index, slot.generation)
        } else {
            let index = u16::try_from(self.slots.len()).expect("wasm handle table overflow");
            self.slots.push(HandleSlot {
                generation: 0,
                value: Some(value),
            });
            Self::pack_handle(index, 0)
        }
    }

    pub fn get(&self, handle: u32) -> Option<&WasmHandle> {
        let (index, generation) = Self::unpack_handle(handle);
        let slot = self.slots.get(index as usize)?;
        if slot.generation == generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    pub fn remove(&mut self, handle: u32) -> Option<WasmHandle> {
        let (index, generation) = Self::unpack_handle(handle);
        let slot = self.slots.get_mut(index as usize)?;
        if slot.generation != generation {
            return None;
        }

        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(index);
        Some(value)
    }

    fn pack_handle(index: u16, generation: u16) -> u32 {
        (u32::from(generation) << 16) | u32::from(index)
    }

    fn unpack_handle(handle: u32) -> (u16, u16) {
        (handle as u16, (handle >> 16) as u16)
    }
}

pub(super) type SharedRuntimeTarget = Arc<Mutex<Option<Weak<Mutex<WasmRuntime>>>>>;

#[derive(Clone)]
pub(super) struct HostState {
    pub handles: HandleTable,
    pub guest_resources: HashMap<GuestResource, usize>,
    pub runtime_target: SharedRuntimeTarget,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            handles: HandleTable::default(),
            guest_resources: HashMap::new(),
            runtime_target: Arc::new(Mutex::new(None)),
        }
    }
}

pub(super) fn insert_wasm_handle(state: &mut HostState, value: WasmHandle) -> u32 {
    if let Some(resource) = value.guest_resource() {
        *state.guest_resources.entry(resource).or_default() += 1;
    }

    state.handles.insert(value)
}

pub(super) fn release_guest_resource_count(
    state: &mut HostState,
    resource: &GuestResource,
) -> bool {
    match state.guest_resources.get_mut(resource) {
        Some(count) if *count > 1 => {
            *count -= 1;
            false
        }
        Some(_) => {
            state.guest_resources.remove(resource);
            true
        }
        None => false,
    }
}

fn guest_drop_symbol(resource: &GuestResource) -> &'static str {
    match resource {
        GuestResource::Object(_) => "koto_plugin_object_drop_v1",
        GuestResource::Iterator(_) => "koto_plugin_iterator_drop_v1",
        GuestResource::NativeFunction(_) => "koto_plugin_native_function_drop_v1",
    }
}

fn guest_drop_user_data(resource: &GuestResource) -> u32 {
    match resource {
        GuestResource::Object(user_data)
        | GuestResource::Iterator(user_data)
        | GuestResource::NativeFunction(user_data) => *user_data,
    }
}

pub(super) fn call_guest_drop_with_caller(
    caller: &mut Caller<'_, HostState>,
    resource: &GuestResource,
) -> std::result::Result<(), wasmi::Error> {
    let Some(drop_fn) = caller
        .get_export(guest_drop_symbol(resource))
        .and_then(Extern::into_func)
    else {
        return Ok(());
    };
    let drop_fn = drop_fn
        .typed::<i32, ()>(&*caller)
        .map_err(|error| wasmi::Error::new(error.to_string()))?;
    drop_fn
        .call(caller, guest_drop_user_data(resource) as i32)
        .map_err(|error| wasmi::Error::new(error.to_string()))
}

pub(super) fn call_guest_drop_with_runtime(
    runtime: &mut WasmRuntime,
    resource: &GuestResource,
) -> crate::Result<()> {
    let Some(drop_fn) = runtime
        .instance
        .get_export(&runtime.store, guest_drop_symbol(resource))
        .and_then(Extern::into_func)
    else {
        return Ok(());
    };
    let drop_fn = drop_fn
        .typed::<i32, ()>(&runtime.store)
        .map_err(|error| crate::Error::from(error.to_string()))?;
    drop_fn
        .call(&mut runtime.store, guest_drop_user_data(resource) as i32)
        .map_err(|error| crate::Error::from(error.to_string()))
}
