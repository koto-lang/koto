use koto_runtime::{prelude::*, Borrow, Ptr, PtrMut, Result};

/// Captures output from Koto in a String
///
/// [KotoWrite] is implemented for OutputCapture, allowing it to be used as stdout and stderr
/// for the Koto runtime.
#[derive(Clone, Debug)]
pub struct OutputCapture {
    output: PtrMut<String>,
}

impl Default for OutputCapture {
    fn default() -> Self {
        Self {
            output: make_ptr_mut!(String::default()),
        }
    }
}

impl OutputCapture {
    /// Returns a [KotoVm] with `stdout` and `stderr` captured by an instance of [OutputCapture]
    pub fn make_vm_with_output_capture() -> (KotoVm, Self) {
        let output_capture = Self::default();

        let vm = KotoVm::with_settings(KotoVmSettings {
            stdout: make_ptr!(output_capture.clone()),
            stderr: make_ptr!(output_capture.clone()),
            ..Default::default()
        });

        (vm, output_capture)
    }

    /// Clears the captured output
    pub fn clear(&mut self) {
        self.output.borrow_mut().clear();
    }

    /// Returns the currently captured output
    pub fn captured_output(&self) -> Borrow<String> {
        self.output.borrow()
    }
}

impl KotoFile for OutputCapture {
    fn id(&self) -> KString {
        "_output_capture_".into()
    }
}

impl KotoRead for OutputCapture {}
impl KotoWrite for OutputCapture {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        let bytes_str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string().into()),
        };
        self.output.borrow_mut().push_str(bytes_str);
        Ok(())
    }

    fn write_line(&self, output: &str) -> Result<()> {
        let mut unlocked = self.output.borrow_mut();
        unlocked.push_str(output);
        unlocked.push('\n');
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}
