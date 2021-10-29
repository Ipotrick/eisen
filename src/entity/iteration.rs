#![macro_use]

#[allow(unused)]
use crate::util::*;

#[allow(unused)]
macro_rules! expand_iteration {
    ($iter:expr, mut $store:expr $(, $($rest:tt)+)?) => {
        {
            unsafe fn forget_lifetime<'a, 'b, T>(reference: &'a mut T) -> &'b mut T {
                let mut_ptr = reference as *mut T;
                &mut*mut_ptr
            }
    
            let new_iter = $iter.filter_map(|tup| {
                let index = *tup.0;
                if $store.has(index) {
                    Some(tup.append(unsafe{forget_lifetime($store.get_mut(index).unwrap())}))
                } else {
                    None
                }
            });

            expand_iteration!(new_iter $(, $($rest)+)?)
        }
    };
    
    ($iter:expr, not $store:expr $(, $($rest:tt)+)?) => {
        {
            let new_iter = $iter.filter(|tup| !$store.has(*tup.0));

            expand_iteration!(new_iter $(, $($rest)+)?)
        }
    };
    
    ($iter:expr, $store:expr $(, $($rest:tt)+)?) => {
        {
            let new_iter = $iter.filter_map(|tup| {
                let index = *tup.0;
                if $store.has(index) {
                    Some(tup.append($store.get(index).unwrap()))
                } else {
                    None
                }
            })

            expand_iteration!(new_iter $(, $($rest)+)?)
        }
    };

    ($iter:expr) => {
        $iter
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! iterate_over_entities_components {
    ($entities:expr, mut $first_store:expr $(, $($rest:tt)+)?) => {
        expand_iteration!($first_store.iter_entity_mut() $(, $($rest)+)?)
        .map(|tup| {
            let index = *tup.0;
            tup.replace_first(EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
        })
    };

    ($entities:expr, $first_store:expr $(,$($rest:tt)+)?) => {
        expand_iteration!($first_store.iter_entity() $(, $($rest)+)?)
            .map(|tup| {
                let index = *tup.0;
                tup.replace_first(EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
            })
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! iterate_over_components {
    (mut $first_store:expr $(,$($rest:tt)+)?) => { 
        expand_iteration!($first_store.iter_entity_mut() $(,$($rest)+)?)
            .map(|tup| tup.pop_front())
    };

    ($first_store:expr $(,$($rest:tt)+)?) => {
        expand_iteration!($first_store.iter_entity() $(,$($rest)+)?)
            .map(|tup| tup.pop_front())
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! iterate_over_entities {
    (mut $first_store:expr $(,$($rest:tt)+)?) => { 
        expand_iteration!($first_store.iter_entity_mut() $(,$($rest)+)?)
            .map(|tup| tup.0)
    };

    ($first_store:expr $(,$($rest:tt)+)?) => {
        expand_iteration!($first_store.iter_entity() $(,$($rest)+)?)
            .map(|tup| tup.0)
    };
}