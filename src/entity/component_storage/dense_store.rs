use super::*;

pub struct DenseStore<T: Default + Clone> {
    sparse_indices: Vec<EntityIndex>,
    dense_indices: Vec<EntityIndex>,
    dense_values: Vec<T>,
}

impl<T: 'static + Default + Clone> GenericComponentStore for DenseStore<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn optimize(&mut self) {}

    fn has(&self, index: EntityIndex) -> bool {
        let index = index as usize;
        index < self.sparse_indices.len() && self.sparse_indices[index] != !(0 as EntityIndex)
    }

    fn rem(&mut self, index: EntityIndex) {
        assert!(self.has(index), "index was {}, val was {}", index, self.sparse_indices[index as usize]);
        self.assure_index(index);
        let dense_index = self.sparse_indices[index as usize] as usize;
        let last_value = self.dense_values.pop().unwrap();
        let last_index = self.dense_indices.pop().unwrap();
        self.dense_values[dense_index] = last_value;
        self.dense_indices[dense_index] = last_index;
        self.sparse_indices[index as usize] = !0;
    }

    fn len(&self) -> usize {
        0
    }
}

impl<T: Default + Clone> DenseStore<T> {
    #[allow(unused)]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.dense_values.iter()
    }
    
    #[allow(unused)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.dense_values.iter_mut()
    }

    #[allow(unused)]
    pub fn iter_entity(&self) -> impl Iterator<Item = (EntityIndex, &T)> {
        self.dense_indices.iter().map(|i|*i).zip(self.dense_values.iter())
    }
    
    #[allow(unused)]
    pub fn iter_entity_mut(&mut self) -> impl Iterator<Item = (EntityIndex, &mut T)> {
        self.dense_indices.iter().map(|i|*i).zip(self.dense_values.iter_mut())
    }

    #[allow(unused)]
    pub fn iter_entity_batch(&self, batch_size: usize) -> impl Iterator<Item = impl Iterator<Item = (EntityIndex, &T)>> {
        
        let n = self.dense_indices.len();

        (0..n).step_by(batch_size).into_iter().map(move |i| {
            self.dense_indices[i..]
                .iter()
                .take(batch_size)
                .map(|i| *i)
                .zip(self.dense_values[i..]
                .iter()
                .take(batch_size))
        })
    }

    #[allow(unused)]
    pub fn iter_entity_mut_batch(&mut self, batch_size: usize) -> impl Iterator<Item = impl Iterator<Item = (EntityIndex, &mut T)>> {

        let n = self.dense_indices.len();
        
        (0..n).step_by(batch_size).into_iter().map(move |i| {
            let forgotten_self = unsafe{std::mem::transmute::<&mut Self, &mut Self>(self)};
            forgotten_self.dense_indices[i..]
                .iter()
                .take(batch_size)
                .map(|i| *i)
                .zip(forgotten_self.dense_values[i..]
                .iter_mut()
                .take(batch_size))
        })
    }

    fn assure_index(&mut self, index: EntityIndex) {
        if index as usize >= self.sparse_indices.len() {
            self.sparse_indices.resize(index as usize + 1, !0);
        }
    }
    #[allow(unused)]

    pub fn sort(&mut self) {
        let mut new_dense_values = Vec::<T>::with_capacity(self.dense_values.len());
        let mut new_dense_indices = Vec::<EntityIndex>::with_capacity(self.dense_values.len());

        for dense_index in self.sparse_indices.iter_mut() {
            *dense_index = 
            if *dense_index != !(0 as EntityIndex) {
                new_dense_indices.push(*dense_index);
                let mut el = T::default();
                std::mem::swap(&mut self.dense_values[*dense_index as usize], &mut el);
                new_dense_values.push(el);
                (new_dense_indices.len() - 1) as EntityIndex
            } else {
                !(0 as EntityIndex)
            }
        }

        self.dense_indices = new_dense_indices;
        self.dense_values = new_dense_values;
    }
}

impl<T: 'static + Default + Clone + Component> ComponentStore<T> for DenseStore<T> {
    type ComponentType = T;

    fn new() -> Self {
        Self{
            sparse_indices: Vec::new(),
            dense_indices: Vec::new(),
            dense_values: Vec::new(),
        }
    }

    fn get(&self, index: EntityIndex) -> Option<&T> {
        if self.has(index) {
            let index = index as usize;
            let dense_index = self.sparse_indices[index] as usize;
            Some(&self.dense_values[dense_index])
        }
        else {
            None
        }
    }
    
    fn get_mut(&mut self, index: EntityIndex) -> Option<&mut T> {
        if self.has(index) {
            let index = index as usize;
            let dense_index = self.sparse_indices[index] as usize;
            Some(&mut self.dense_values[dense_index])
        }
        else {
            None
        }
    }

    fn set(&mut self, index: EntityIndex, value: T) {
        assert!(self.has(index));
        let index = index as usize;
        let dense_index = self.sparse_indices[index] as usize;
        self.dense_values[dense_index] = value;
    }

    fn add(&mut self, index: EntityIndex, value: T) {
        assert!(!self.has(index));
        self.assure_index(index);
        self.dense_values.push(value);
        self.dense_indices.push(index);
        self.sparse_indices[index as usize] = self.dense_indices.len() as EntityIndex - 1;
    }
}

use async_std::sync::RwLock;

pub trait ComponentStoreAccessor: {
    fn exec(&self, f: &mut dyn FnMut(&mut dyn GenericComponentStore) -> ());

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn as_any_ref(&self) -> &dyn Any;
}

impl<T: 'static + Default + Clone> ComponentStoreAccessor for Arc<RwLock<LinearStore<T>>> {
    fn exec(&self, f: &mut dyn FnMut(&mut dyn GenericComponentStore) -> ()) {
        let mut guard = spin_on!(self.try_write());

        let generic_self: &mut dyn GenericComponentStore = &mut *guard;

        f(generic_self);
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

impl<T: 'static + Default + Clone> ComponentStoreAccessor for Arc<RwLock<DenseStore<T>>> {
    fn exec(&self, f: &mut dyn FnMut(&mut dyn GenericComponentStore) -> ()) {
        let mut guard = spin_on!(self.try_write());

        let generic_self: &mut dyn GenericComponentStore = &mut *guard;

        f(generic_self);
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}