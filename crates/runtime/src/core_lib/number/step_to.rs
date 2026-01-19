use crate::{Result, prelude::*};

#[derive(Clone, Debug)]
pub struct StepToI64Iterator {
    target: i64,
    step_by: i64,
    steps_to_target: i64,
}

impl StepToI64Iterator {
    pub fn new(start: i64, target: i64, step_by: i64) -> Self {
        let steps_to_target = (target - start).abs() / step_by;
        let step_by = if target < start { -step_by } else { step_by };
        let target = start + step_by * steps_to_target;

        Self {
            target,
            step_by,
            steps_to_target,
        }
    }
}

impl KotoIterator for StepToI64Iterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        if self.steps_to_target >= 0 {
            let result = self.target;
            self.target -= self.step_by;
            self.steps_to_target -= 1;
            Some(KIteratorOutput::Value(result.into()))
        } else {
            None
        }
    }
}

impl Iterator for StepToI64Iterator {
    type Item = KIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        if self.steps_to_target >= 0 {
            let result = self.target - self.step_by * self.steps_to_target;
            self.steps_to_target -= 1;
            Some(KIteratorOutput::Value(result.into()))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = (self.steps_to_target + 1) as usize;
        (hint, Some(hint))
    }
}

#[derive(Clone, Debug)]
pub struct StepToF64Iterator {
    target: f64,
    step_by: f64,
    steps_to_target: f64,
}

impl StepToF64Iterator {
    pub fn new(start: f64, target: f64, step_by: f64) -> Self {
        let steps_to_target = ((target - start).abs() / step_by).floor();
        let step_by = if target < start { -step_by } else { step_by };
        let target = start + step_by * steps_to_target;

        Self {
            target,
            step_by,
            steps_to_target,
        }
    }
}

impl KotoIterator for StepToF64Iterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        if self.steps_to_target >= 0.0 {
            let result = self.target;
            self.target -= self.step_by;
            self.steps_to_target -= 1.0;
            Some(KIteratorOutput::Value(result.into()))
        } else {
            None
        }
    }
}

impl Iterator for StepToF64Iterator {
    type Item = KIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        if self.steps_to_target >= 0.0 {
            let result = self.target - self.step_by * self.steps_to_target;
            self.steps_to_target -= 1.0;
            Some(KIteratorOutput::Value(result.into()))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = (self.steps_to_target + 1.0) as usize;
        (hint, Some(hint))
    }
}
