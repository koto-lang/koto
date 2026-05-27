//! The `task` core library module

use crate::{Result, prelude::*};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

static MODULE_NAME: &str = "core.task";

/// Initializes the `task` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type(MODULE_NAME);

    result.add_fn("spawn", |ctx| {
        let expected_error = "|Callable, Any...|";

        match ctx.args() {
            [function, args @ ..] if function.is_callable() => {
                let function = function.clone();
                let args = args.to_vec();
                let mut task = ctx
                    .vm
                    .call_function_without_awaiting_as_task(function, args.as_slice())?;

                match task.poll()? {
                    KTaskPoll::Ready(KValue::Task(task)) => Ok(task.into()),
                    KTaskPoll::Ready(value) => Ok(ctx.spawn_task(KTask::with_value(value))?.into()),
                    KTaskPoll::Pending => Ok(task.into()),
                }
            }
            args => unexpected_args(expected_error, args),
        }
    });

    result.add_fn("is_complete", |ctx| {
        let expected_error = "|Task|";

        match ctx.instance_and_args(|value| matches!(value, KValue::Task(_)), expected_error)? {
            (KValue::Task(task), []) => Ok(task.is_complete().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("is_failed", |ctx| {
        let expected_error = "|Task|";

        match ctx.instance_and_args(|value| matches!(value, KValue::Task(_)), expected_error)? {
            (KValue::Task(task), []) => Ok(task.is_failed().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("is_active", |ctx| {
        let expected_error = "|Task|";

        match ctx.instance_and_args(|value| matches!(value, KValue::Task(_)), expected_error)? {
            (KValue::Task(task), []) => Ok(task.is_active().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("status", |ctx| {
        let expected_error = "|Task|";

        match ctx.instance_and_args(|value| matches!(value, KValue::Task(_)), expected_error)? {
            (KValue::Task(task), []) if task.is_complete() => Ok("complete".into()),
            (KValue::Task(task), []) if task.is_failed() => Ok("failed".into()),
            (KValue::Task(_), []) => Ok("active".into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("join", |ctx| {
        let expected_error = "|Task..., or List[Task], or Tuple[Task]|";

        let tasks = tasks_from_args(ctx.args(), expected_error, true)?;
        Ok(ctx.spawn_future(TaskJoin::new(tasks))?.into())
    });

    result.add_fn("select", |ctx| {
        let expected_error = "|Task..., or List[Task], or Tuple[Task]|";

        let tasks = tasks_from_args(ctx.args(), expected_error, false)?;
        Ok(ctx.spawn_future(TaskSelect::new(tasks))?.into())
    });

    result.add_fn("timeout", |ctx| {
        let expected_error = "|Task, Number >= 0|";

        match ctx.args() {
            [KValue::Task(task), KValue::Number(seconds)] if *seconds >= 0.0 => {
                let sleep = ctx.sleep(Duration::from_secs_f64(f64::from(seconds)))?;
                Ok(ctx
                    .spawn_future(TaskTimeout {
                        task: task.clone(),
                        sleep,
                    })?
                    .into())
            }
            args => unexpected_args(expected_error, args),
        }
    });

    result.add_fn("sleep", |ctx| {
        let expected_error = "|Number|";

        match ctx.args() {
            [KValue::Number(n)] if *n >= 0.0 => {
                Ok(ctx.sleep(Duration::from_secs_f64(f64::from(n)))?.into())
            }
            args => unexpected_args(expected_error, args),
        }
    });

    result
}

fn tasks_from_args(args: &[KValue], expected_error: &str, allow_empty: bool) -> Result<Vec<KTask>> {
    let values: Vec<KValue> = match args {
        [KValue::List(list)] => list.data().iter().cloned().collect(),
        [KValue::Tuple(tuple)] => tuple.iter().cloned().collect(),
        _ => args.to_vec(),
    };

    if values.is_empty() && !allow_empty {
        return unexpected_args(expected_error, args);
    }

    let mut result = Vec::with_capacity(values.len());

    for value in values {
        match value {
            KValue::Task(task) => result.push(task),
            unexpected => return unexpected_type("Task", &unexpected),
        }
    }

    Ok(result)
}

struct TaskJoin {
    tasks: Vec<KTask>,
    results: Vec<Option<KValue>>,
}

impl TaskJoin {
    fn new(tasks: Vec<KTask>) -> Self {
        Self {
            results: vec![None; tasks.len()],
            tasks,
        }
    }
}

impl Future for TaskJoin {
    type Output = Result<KValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut all_ready = true;

        for i in 0..self.tasks.len() {
            if self.results[i].is_some() {
                continue;
            }

            match self.tasks[i].poll_with_context(context)? {
                KTaskPoll::Ready(value) => {
                    self.results[i] = Some(value);
                }
                KTaskPoll::Pending => {
                    all_ready = false;
                }
            }
        }

        if all_ready {
            let result = self
                .results
                .iter()
                .map(|value| value.clone().expect("task result missing"))
                .collect::<ValueVec>();

            Poll::Ready(Ok(KList::with_data(result).into()))
        } else {
            Poll::Pending
        }
    }
}

struct TaskSelect {
    tasks: Vec<KTask>,
}

impl TaskSelect {
    fn new(tasks: Vec<KTask>) -> Self {
        Self { tasks }
    }
}

impl Future for TaskSelect {
    type Output = Result<KValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        for task in &mut self.tasks {
            match task.poll_with_context(context)? {
                KTaskPoll::Ready(value) => return Poll::Ready(Ok(value)),
                KTaskPoll::Pending => {}
            }
        }

        Poll::Pending
    }
}

struct TaskTimeout {
    task: KTask,
    sleep: KTask,
}

impl Future for TaskTimeout {
    type Output = Result<KValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if let KTaskPoll::Ready(value) = self.task.poll_with_context(context)? {
            return Poll::Ready(Ok(value));
        }

        if let KTaskPoll::Ready(_) = self.sleep.poll_with_context(context)? {
            return Poll::Ready(runtime_error!("task timed out"));
        }

        Poll::Pending
    }
}
