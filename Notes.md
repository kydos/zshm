# SHM API

## Default Shm Provider Construction

Conceptually, in order to create a default-provider via the ShmProviderBuilder we need to know the size and the default alignment. Thus one would expect that the API would look like:

```rust
    pub fn default_backend(size: usize, align: u8)
```

The current ShmProviderBuilder is as follows:

```rust
    pub fn default_backend<What>(what: What) -> ShmProviderBuilderWithDefaultBackend<What> {
        ShmProviderBuilderWithDefaultBackend { what }
    }
```

This is powerful since it allows us to create as many way as we wish to create a default provider by implementation, but it makes the API hard to understand and document based on its signature.

For instance, if we look at the following code:

```rust
  let _ = ShmProviderBuilder::default_backend((1024, AllocAlignment::ALIGN_4_BYTES))
        .wait()
        .unwrap();

    let shm_provider = ShmProviderBuilder::default_backend(typed_layout.layout())
        .wait()
        .unwrap();
```

Both of these are correct and compile, yet it is hard to figure out that this is legal code by looking at the signature of `default_backend`.

Is this generalisation worth the complexity? I am a big fan of API don't require documentation, those should be self explanatory.

## Typed Buffer
When allocating a typed buffer we get:

```rust
    let mut buf: Typed<SharedData, ZShmMut> = shm_provider.alloc(typed_layout).wait().unwrap();
```

Then to allow for cloning we need to morph it into something that is not mutable by:

```rust
    let mut buf: Typed<_, ZShm> = buf.into();
```

The current API uses `into` to change a typed buffer from `ZShmMut` to `ZShm`, as shown above. This forces the user to know the poper signature of the desired return type. Can't we do with a dedicated method to avoid this issue? Once again this will make the API easier to follow.

The other question I have is why the buf needs to be `mut` when morphed. I see that the following requires a mut buffer, but why? Is it for concurrency guarantees?

```rust
    let shared_data = unsafe { buf.as_mut_unchecked() };
```rust
