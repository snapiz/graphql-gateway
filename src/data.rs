use fnv::FnvHashMap;
use std::any::{Any, TypeId};

#[derive(Default)]
pub struct Data(FnvHashMap<TypeId, Box<dyn Any + Sync + Send>>);

impl Data {
    pub fn insert<D: Any + Send + Sync>(&mut self, data: D) {
        self.0.insert(TypeId::of::<D>(), Box::new(data));
    }

    pub fn get<D: Any + Send + Sync>(&self) -> Option<&D> {
        self.0
            .get(&TypeId::of::<D>())
            .and_then(|d| d.downcast_ref::<D>())
    }
}
