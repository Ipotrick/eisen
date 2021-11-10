use super::*;

const fn get_page_index(index: EntityIndex, page_exponent: usize) -> usize {
    index as usize >> page_exponent
}

const fn get_page_offset(index: EntityIndex, page_mask: usize) -> usize {
    index as usize & page_mask
}

const fn get_page_mask(page_exponent: usize) -> usize {
    !(usize::MAX << page_exponent)
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

    fn len(&self) -> usize {
        0
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
    pub fn iter_entity_batch(&self, batch_size: usize) -> impl Iterator<Item = impl Iterator<Item = (EntityIndex, &T)>> {
        let n = self.pages.len();

        (0..n).into_iter()
            .map(|page_index|{
                self.pages[page_index].iter_entity(page_index).take(PAGE_SIZE)
            })
    }

    #[allow(unused)]
    pub fn iter_entity_mut_batch(&mut self, batch_size: usize) -> impl Iterator<Item = impl Iterator<Item = (EntityIndex, &mut T)>> {
        let n = self.pages.len();

        (0..n).into_iter()
            .map(|page_index|{
                let forgotten_self = unsafe{std::mem::transmute::<&mut Self, &mut Self>(self)};
                forgotten_self.pages[page_index].iter_entity_mut(page_index).take(PAGE_SIZE)
            })
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

impl<T: Default + Clone + Component> ComponentStore<T> for LinearStore<T> {
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