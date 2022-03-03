#![macro_use]

#[allow(unused)]
pub use crate::util::*;
pub use crate::entity::component_storage::*;

#[allow(unused)]
#[macro_export]
pub(crate) fn forget_lifetime_mut<'a, 'b, T>(reference: &'a mut T) -> &'b mut T {
    unsafe{std::mem::transmute::<&'a mut T, &'b mut T>(reference)}
}

#[allow(unused)]
#[macro_export]
pub(crate) fn forget_lifetime<'a, 'b, T>(reference: &'a T) -> &'b T {
    unsafe{std::mem::transmute::<&'a T, &'b T>(reference)}
}

#[allow(unused)]
#[macro_export]
macro_rules! expand_iteration {
    ($iter:expr, mut $store:expr $(, $($rest:tt)+)?) => {
        eisen::expand_iteration!(
            $iter.filter_map(|tup| {
                let index = tup.0;
                Some(tup.append(eisen::forget_lifetime_mut($store.get_mut(index)?)))
            })
            $(, $($rest)+)?
        )
    };
    
    ($iter:expr, not $store:expr $(, $($rest:tt)+)?) => {
        eisen::expand_iteration!($iter.filter(|tup| !$store.has(tup.0)) $(, $($rest)+)?)
    };
    
    ($iter:expr, $store:expr $(, $($rest:tt)+)?) => {
        eisen::expand_iteration!(
            $iter.filter_map(|tup| {
                let index = tup.0;
                Some(tup.append($store.get(index)?))
            })
            $(, $($rest)+)?
        )
    };

    ($iter:expr) => {
        $iter
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! erase_lifetime_check {
    (mut $first:ident $(, $($rest:tt)+)?) => {
        let $first = crate::entity::iteration::forget_lifetime_mut($first);
        $(eisen::erase_lifetime_check!($($rest)+))?
    };

    (not $first:ident $(, $($rest:tt)+)?) => {
        let $first = crate::entity::iteration::forget_lifetime($first);
        $(eisen::erase_lifetime_check!($($rest)+))?
    };

    ($first:ident $(, $($rest:tt)+)?) => {
        let $first = crate::entity::iteration::forget_lifetime(&*$first);
        $(eisen::erase_lifetime_check!($($rest)+))?
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! parallel_over_entities {
    ($(note: $note:literal;)? runtime: $runtime:expr; batch_size: $batch_size:expr; closure: $closure:expr; entities: $entities:ident; stores: $first_store:ident $(,$($rest:tt)+)?) => {
        async { 
            let waiter = crate::sync::AtomicWaiter::new();
            eisen::erase_lifetime_check!($first_store);

            $first_store.iter_entity_batch($batch_size)
            .for_each(|batch_iter|{
                $(eisen::erase_lifetime_check!($($rest)+);)?
                eisen::erase_lifetime_check!($entities);
                let dep = waiter.make_dependency();
                let func = || { 
                    profiling::scope!("parallel_over_entities" $(,$note)?);
                    let _d = dep;
                    eisen::expand_iteration!(batch_iter $(,$($rest)+)?)
                        .map(|tup| {
                            let index = tup.0;
                            tup.replace_first(EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
                        })
                        .for_each($closure);
                };
    
                $runtime.exec(func);
            });

            waiter
        }
    };

    ($(note: $note:literal;)? runtime: $runtime:expr; batch_size: $batch_size:expr; closure: $closure:expr; entities: $entities:ident; stores: mut $first_store:ident $(,$($rest:tt)+)?) => {
        async {
            let waiter = crate::sync::AtomicWaiter::new();
            eisen::erase_lifetime_check!(mut $first_store);

            $first_store.iter_entity_mut_batch($batch_size)
                .for_each(|batch_iter|{
                    $(eisen::erase_lifetime_check!($($rest)+);)?
                    eisen::erase_lifetime_check!($entities);
                    let dep = waiter.make_dependency();
                    let func = move || { 
                        profiling::scope!("parallel_over_entities" $(,$note)?);
                        let _d = dep;
                        eisen::expand_iteration!(batch_iter $(,$($rest)+)?)
                            .map(|tup| {
                                let index = tup.0;
                                tup.replace_first(EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
                            })
                            .for_each($closure);
                    };
        
                    $runtime.exec(func);
                });

            waiter.await
        }
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! iterate_over_entities {
    (entities: $entities:expr; stores: mut $first_store:expr $(,$($rest:tt)+)?) => { 
        eisen::expand_iteration!($first_store.iter_entity_mut() $(, $($rest)+)?)
            .map(|tup|{
                let index = tup.0;
                tup.replace_first(eisen::entity::EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
            })
    };

    (entities: $entities:expr; stores: $first_store:expr $(,$($rest:tt)+)?) => {
        eisen::expand_iteration!($first_store.iter_entity() $(, $($rest)+)?)
            .map(|tup| {
                let index = tup.0;
                tup.replace_first(eisen::entity::EntityHandle{index: index, version: $entities.version_of(index).unwrap()})
            })
    };
    
    (stores: mut $first_store:expr $(,$($rest:tt)+)?) => { 
        eisen::expand_iteration!($first_store.iter_entity_mut() $(, $($rest)+)?)
            .map(|tup| {
                tup.pop_front()
            })
    };

    (stores: $first_store:expr $(,$($rest:tt)+)?) => { 
        eisen::expand_iteration!($first_store.iter_entity() $(, $($rest)+)?)
            .map(|tup| {
                tup.pop_front()
            })
    };
}