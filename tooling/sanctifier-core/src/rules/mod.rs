pub trait Rule {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn check(&self, contract_code: &str) -> Vec<String>;
}

pub struct Registry {
    rules: Vec<Box<dyn Rule>>,
}

impl Registry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: impl Rule + 'static) {
        self.rules.push(Box::new(rule));
    }
}
