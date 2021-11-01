#![macro_use]

#[allow(unused)]
use crate::util::*;

#[allow(unused)]
pub(crate) fn forget_lifetime_mut<'a, 'b, T>(reference: &'a mut T) -> &'b mut T {
    unsafe{std::mem::transmute::<&'a mut T, &'b mut T>(reference)}
}

#[allow(unused)]
pub(crate) fn forget_lifetime<'a, 'b, T>(reference: &'a T) -> &'b T {
    unsafe{std::mem::transmute::<&'a T, &'b T>(reference)}
}

#[allow(unused)]
macro_rules! expand_iteration {
    ($iter:expr, mut $store:expr $(, $($rest:tt)+)?) => {
        {    
            let new_iter = $iter.filter_map(|tup| {
                let index = tup.0;
                Some(tup.append(crate::entity::iteration::forget_lifetime_mut($store.get_mut(index)?)))
            });

            expand_iteration!(new_iter $(, $($rest)+)?)
        }
    };
    
    ($iter:expr, not $store:expr $(, $($rest:tt)+)?) => {
        {
            let new_iter = $iter.filter(|tup| !$store.has(tup.0));
            expand_iteration!(new_iter $(, $($rest)+)?)
        }
    };
    
    ($iter:expr, $store:expr $(, $($rest:tt)+)?) => {
        {
            let new_iter = $iter.filter_map(|tup| {
                let index = tup.0;
                Some(tup.append($store.get(index)?))
            });

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
            let index = tup.0;
            tup.replace_first(EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
        })
    };

    ($entities:expr, $first_store:expr $(,$($rest:tt)+)?) => {
        expand_iteration!($first_store.iter_entity() $(, $($rest)+)?)
            .map(|tup| {
                let index = tup.0;
                tup.replace_first(EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
            })
    };
}

#[allow(unused)]
macro_rules! wash_lifetime {

    (mut $first:ident, $($rest:tt)+) => {
        let $first = crate::entity::iteration::forget_lifetime_mut($first);
        wash_lifetime!($($rest)+)
    };

    (not $first:ident, $($rest:tt)+) => {
        let $first = crate::entity::iteration::forget_lifetime($first);
        wash_lifetime!($($rest)+)
    };

    ($first:ident, $($rest:tt)+) => {
        let $first = crate::entity::iteration::forget_lifetime(&*$first);
        wash_lifetime!($($rest)+)
    };
    
    (mut $first:ident) => {
        let $first = crate::entity::iteration::forget_lifetime_mut($first);
    };

    (not $first:ident) => {
        let $first = crate::entity::iteration::forget_lifetime($first);
    };
    
    ($first:ident) => {
        let $first = crate::entity::iteration::forget_lifetime($first);
    };
}

#[allow(unused)]
macro_rules! borrow_multi {

    (mut $first:ident, $($rest:tt)+) => {
        let $first = $first;
        wash_lifetime!($($rest)+)
    };

    (not $first:ident, $($rest:tt)+) => {
        let $first = $first;
        wash_lifetime!($($rest)+)
    };

    ($first:ident, $($rest:tt)+) => {
        let $first = $first;
        wash_lifetime!($($rest)+)
    };
    
    (mut $first:ident) => {
        let $first = $first;
    };

    (not $first:ident) => {
        let $first = $first;
    };
    
    ($first:ident) => {
        let $first = $first;
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! parallel_over_entities {
    (runntime: $runntime:expr; batch_size: $batch_size:expr; closure: $closure:expr; entities: $entities:ident; stores: $first_store:ident $(,$($rest:tt)+)?) => {
        async {
            let waiter = crate::sync::AtomicWaiter::new();
            let $first_store = crate::entity::iteration::forget_lifetime($first_store);
            let mut batches = $first_store.iter_entity_batch($batch_size);
            while let Some(sub_store_iter) = batches.pop() {
                let _first_dummy = &0;
                wash_lifetime!(_first_dummy $(,$($rest)+)?);
                let clo = $closure.clone();
                let ent = crate::entity::iteration::forget_lifetime($entities);
                let dep = waiter.make_dependency();
                let func = move || {
                    let _d = dep;
                    let iter = expand_iteration!(sub_store_iter $(,$($rest)+)?)
                        .map(|tup| {
                            let index = tup.0;
                            tup.replace_first(EntityHandle{index: index, version: ent.version_of(index).unwrap()})
                        });
                    
                    iter.for_each(clo);
                };
    
                $runntime.exec(func);
            }

            waiter
        }
    };

    (runntime: $runntime:expr; batch_size: $batch_size:expr; closure: $closure:expr; entities: $entities:ident; stores: mut $first_store:ident $(,$($rest:tt)+)?) => {
        async {
            let waiter = crate::sync::AtomicWaiter::new();
            let $first_store = crate::entity::iteration::forget_lifetime_mut($first_store);
            let mut batches = $first_store.iter_entity_mut_batch($batch_size);
            while let Some(sub_store_iter) = batches.pop() {
                let _first_dummy = &0;
                wash_lifetime!(_first_dummy $(,$($rest)+)?);
                let clo = $closure.clone();
                let ent = crate::entity::iteration::forget_lifetime($entities);
                let dep = waiter.make_dependency();
                let func = move || { 
                    let _d = dep;
                    let iter = expand_iteration!(sub_store_iter $(,$($rest)+)?)
                        .map(|tup| {
                            let index = tup.0;
                            tup.replace_first(EntityHandle{index: index, version: ent.version_of(index).unwrap()})
                        });
                    
                    iter.for_each(clo);
                };
    
                $runntime.exec(func);
            }

            waiter.await
        }
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