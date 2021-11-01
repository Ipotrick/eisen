use std::any::*;
use std::sync::Arc;

use crate::entity::handle::*;

const fn get_page_index(index: EntityIndex, page_exponent: usize) -> usize {
    index as usize >> page_exponent
}

const fn get_page_offset(index: EntityIndex, page_mask: usize) -> usize {
    index as usize & page_mask
}

const fn get_page_mask(page_exponent: usize) -> usize {
    !(usize::MAX << page_exponent)
}

pub trait GenericComponentStore {
    fn optimize(&mut self);

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn has(&self, index: EntityIndex) -> bool;

    fn rem(&mut self, index: EntityIndex);
}

pub trait ComponentStore<T: Default + Clone> {
    type ComponentType;

    fn new() -> Self;

    fn get(&self, index: EntityIndex) -> Option<&T>;

    fn get_mut(&mut self, index: EntityIndex) -> Option<&mut T>;

    fn set(&mut self, index: EntityIndex, value: T);

    fn add(&mut self, index: EntityIndex, value: T);
}

const PAGE_EXPONENT: usize = 7;
const PAGE_SIZE: usize = 1 << PAGE_EXPONENT;
const PAGE_MASK: usize = get_page_mask(PAGE_EXPONENT);

#[derive(Clone)]
struct Page<T: Default + Clone, const N: usize> {
    slots: [T; N],
    slot_used: [bool; N],
    len: usize,
}

impl<T: Default + Clone, const N: usize> Page<T,N> {
    fn new() -> Self {
        Self{
            slots: [(); N].map(|_| T::default()),
            slot_used: [false; N],
            len: 0,
        }
    }

    #[allow(unused)]
    fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter()
            .zip(self.slot_used.iter())
            .filter(|(_, used)| **used)
            .map(|(slot, _)| slot)
    }

    #[allow(unused)]
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut()
            .zip(self.slot_used.iter())
            .filter(|(_, used)| **used)
            .map(|(slot, _)| slot)
    }

    #[allow(unused)]
    fn iter_entity(&self, page_index: usize) -> impl Iterator<Item = (EntityIndex, &T)> {
        self.slots.iter()
            .zip(self.slot_used.iter())
            .enumerate()
            .filter(|(_, (_, used))| **used)
            .map(move |(index, (slot, _))| ((index + (page_index << PAGE_EXPONENT)) as EntityIndex,  slot))
    }

    #[allow(unused)]
    fn iter_entity_mut(&mut self, page_index: usize) -> impl Iterator<Item = (EntityIndex, &mut T)> {
        self.slots.iter_mut()
            .zip(self.slot_used.iter())
            .enumerate()
            .filter(|(_, (_, used))| **used)
            .map(move |(index, (slot, _))| ((index + (page_index << PAGE_EXPONENT)) as EntityIndex,  slot))
    }
}

pub struct LinearStore<T: Default + Clone> {
    pages: Vec<Page<T, PAGE_SIZE>>,
}

impl<T: 'static + Default + Clone> GenericComponentStore for LinearStore<T> {

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn optimize(&mut self) {}

    fn has(&self, index: EntityIndex) -> bool {
        let page_index = get_page_index(index, PAGE_EXPONENT);
        let page_offset = get_page_offset(index, PAGE_MASK);
        self.has_split(page_index, page_offset)
    }

    fn rem(&mut self, index: EntityIndex) {
        let page_index = get_page_index(index, PAGE_EXPONENT);
        let page_offset = get_page_offset(index, PAGE_MASK);
        assert!(self.has_split(page_index, page_offset), "tried to remove non existing component of an entity");
        let page = &mut self.pages[page_index];
        page.slots[page_offset] = T::default();
        page.slot_used[page_offset] = false;
        page.len -= 1;
    }
}

impl<T: Default + Clone> LinearStore<T> {
    #[allow(unused)]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.pages.iter().filter(|page| page.len > 0).flat_map(|page| page.iter())
    }
    
    #[allow(unused)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.pages.iter_mut().filter(| page| page.len > 0).flat_map(|page| page.iter_mut())
    }

    #[allow(unused)]
    pub fn iter_entity(&self) -> impl Iterator<Item = (EntityIndex, &T)> {
        self.pages
            .iter()
            .enumerate()
            .filter(|(_,  page)| page.len > 0)
            .map(|(entity,  page)| (entity, page))
            .flat_map(|(page_index, page)| page.iter_entity(page_index))
    }
    
    #[allow(unused)]
    pub fn iter_entity_mut(&mut self) -> impl Iterator<Item = (EntityIndex, &mut T)> {
        self.pages
            .iter_mut()
            .enumerate()
            .filter(|(_,  page)| page.len > 0)
            .map(|(entity,  page)| (entity, page))
            .flat_map(|(page_index, page)| page.iter_entity_mut(page_index))
    }

    #[allow(unused)]
    pub fn iter_entity_batch(&self, batch_size: usize) -> Vec<impl Iterator<Item = (EntityIndex, &T)>> {
        let mut vec = Vec::new();
        
        let n = self.pages.len();

        for page_index in 0..n {
            vec.push(self.pages[page_index].iter_entity(page_index).take(PAGE_SIZE));
        }
        vec
    }

    #[allow(unused)]
    pub fn iter_entity_mut_batch(&mut self, batch_size: usize) -> Vec<impl Iterator<Item = (EntityIndex, &mut T)>> {
        let mut vec = Vec::new();
        
        fn forget_lifetime_mut<'a, 'b, T>(reference: &'a mut T) -> &'b mut T {
            let ptr = reference as *mut T;
            unsafe{&mut*ptr} 
        }

        let n = self.pages.len();

        for page_index in 0..n {
            let forgotten_self = unsafe{std::mem::transmute::<&mut Self, &mut Self>(self)};
            vec.push(forgotten_self.pages[page_index].iter_entity_mut(page_index).take(PAGE_SIZE));
        }
        vec
    }

    fn assure_page(&mut self, page_index: usize) {
        if self.pages.len() <= page_index {
            self.pages.resize(page_index + 1, Page::new());
        }
    }

    fn has_split(&self, page_index: usize, page_offset: usize) -> bool {
        page_index < self.pages.len() && self.pages[page_index].slot_used[page_offset]
    }
}

impl<T: Default + Clone> ComponentStore<T> for LinearStore<T> {
    type ComponentType = T;

    fn new() -> Self {
        Self{
            pages: Vec::new(),
        }
    }

    fn get(&self, index: EntityIndex) -> Option<&T> {
        let page_index = get_page_index(index, PAGE_EXPONENT);
        let page_offset = get_page_offset(index, PAGE_MASK);
        if self.has_split(page_index,page_offset) {
            Some(&self.pages[page_index].slots[page_offset])
        } else {
            None
        }
    }

    fn get_mut(&mut self, index: EntityIndex) -> Option<&mut T> {
        let page_index = get_page_index(index, PAGE_EXPONENT);
        let page_offset = get_page_offset(index, PAGE_MASK);
        if self.has_split(page_index,page_offset) {
            Some(&mut self.pages[page_index].slots[page_offset])
        }else {
            None
        }
    }

    fn set(&mut self, index: EntityIndex, value: T) {
        let page_index = get_page_index(index, PAGE_EXPONENT);
        let page_offset = get_page_offset(index, PAGE_MASK);
        assert!(self.has_split(page_index,page_offset));
        self.pages[page_index].slots[page_offset] = value;
    }

    fn add(&mut self, index: EntityIndex, value: T) {
        let page_index = get_page_index(index, PAGE_EXPONENT);
        let page_offset = get_page_offset(index, PAGE_MASK);
        assert!(!self.has_split(page_index, page_offset), "tried to add a component to an entity that allready has the given component");
        self.assure_page(page_index);
        let page = &mut self.pages[page_index];
        page.slots[page_offset] = value;
        page.slot_used[page_offset] = true;
        page.len += 1;
    }
}

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
    pub fn iter_entity_batch(&self, batch_size: usize) -> Vec<impl Iterator<Item = (EntityIndex, &T)>> {
        let mut vec = Vec::new();
        
        let n = self.dense_indices.len();

        for start in (0..n).step_by(batch_size) {
            println!("start is at: {}", start);
            vec.push(self.dense_indices[start..].iter().map(|i| *i).take(batch_size).zip(self.dense_values[start..].iter().take(batch_size)));
        }
        vec
    }

    #[allow(unused)]
    pub fn iter_entity_mut_batch(&mut self, batch_size: usize) -> Vec<impl Iterator<Item = (EntityIndex, &mut T)>> {
        let mut vec = Vec::new();

        let n = self.dense_indices.len();

        for start in (0..n).step_by(batch_size) {
            let forgotten_self = unsafe{std::mem::transmute::<&mut Self, &mut Self>(self)};
            vec.push(forgotten_self.dense_indices[start..].iter().map(|i| *i).take(batch_size).zip(forgotten_self.dense_values[start..].iter_mut().take(batch_size)));
        }
        vec
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

impl<T: 'static + Default + Clone> ComponentStore<T> for DenseStore<T> {
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

pub trait Component : Clone + Default + Sync + Send
{
    type Storage : ComponentStore<Self>;
}