#[macro_export]
macro_rules! newtype {
    ($vis:vis struct $name:ident($wrapped:ty)) => {
        $vis struct $name($wrapped);

        impl From<$name> for $wrapped {
            fn from(newtype_instance: $name) -> Self {
                newtype_instance.0
            }
        }

        impl From<$wrapped> for $name {
            fn from(inner: $wrapped) -> Self {
                Self(inner)
            }
        }

        impl std::ops::Deref for $name {
            type Target = $wrapped;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}
