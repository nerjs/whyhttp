use crate::expectation::Expectation;


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectationId(u32);

pub struct Worker {
    previous_id: u32,
    expectations: Vec<(ExpectationId, Expectation)>,
}

impl Worker {
    pub fn create_next(&mut self) -> ExpectationId {
        let next_id = self.previous_id.wrapping_add(1);
        let id = ExpectationId(next_id);
        self.previous_id = next_id;
        self.expectations.push((id.clone(), Expectation::default()));
        id
    }
}
