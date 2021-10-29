pub trait TupleExtrasT<T> {
    type AppendResultType;
    type FirstReplaceResultType;

    fn append(self, t: T) -> Self::AppendResultType;
    fn replace_first(self, r: T) -> Self::FirstReplaceResultType;
}

pub trait TupleExtras {
    type PopFrontResultType;

    fn pop_front(self) -> Self::PopFrontResultType;
}

impl<T> TupleExtrasT<T> for () {
    type AppendResultType = (T,);
    type FirstReplaceResultType = ();

    fn append(self, t: T) -> Self::AppendResultType {
        (t,)
    }

    fn replace_first(self, _r: T) -> Self::FirstReplaceResultType {
        self
    }
}

impl TupleExtras for () {
    type PopFrontResultType = ();

    fn pop_front(self) -> Self::PopFrontResultType {
        self
    }
}

macro_rules! impl_tuple_extrasT {
    ( () ) => {};
    ( ( $t0:ident $(, $types:ident)* ) ) => {
        impl<$t0, $($types,)* T> TupleExtrasT<T> for ($t0, $($types,)*) {
            type AppendResultType = ($t0, $($types,)* T,);
            type FirstReplaceResultType = (T, $($types,)*);

            fn append(self, t: T) -> Self::AppendResultType {
                let ($t0, $($types,)*) = self;
                ($t0, $($types,)* t,)
            }

            fn replace_first(self, r: T) -> Self::FirstReplaceResultType {
                let ($t0, $($types,)*) = self;
                (r, $($types,)*)
            }
        }

        // Recurse for one smaller size:
        impl_tuple_extrasT! { ($($types),*) }
    };
}

macro_rules! impl_tuple_extras {
    ( () ) => {};
    ( ( $t0:ident $(, $types:ident)* ) ) => {
        impl<$t0, $($types,)*> TupleExtras for ($t0, $($types,)*) {
            type PopFrontResultType = ($($types,)*);

            fn pop_front(self) -> Self::PopFrontResultType {
                let ($t0, $($types,)*) = self;
                ($($types,)*)
            }
        }

        // Recurse for one smaller size:
        impl_tuple_extras! { ($($types),*) }
    };
}

impl_tuple_extrasT! {
    (_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15)
}

impl_tuple_extras! {
    (_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15)
}