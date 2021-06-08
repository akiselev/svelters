use crate::types::*;

pub struct Signals {
    pub include_abstract: bool,
}

impl Default for Signals {
    fn default() -> Signals {
        Signals {
            include_abstract: true,
        }
    }
}

impl Visitor for Signals {
    fn visit_class(&mut self, class: &Class) {
        if self.include_abstract || !class.is_abstract() {
            println!("{}", class);
        }
    }

    fn visit_signal(&mut self, method: &Method) {
        let left = format!("    {}", method);
        println!("{:<80}{:>20}", left, method.tag_str());
    }
}
