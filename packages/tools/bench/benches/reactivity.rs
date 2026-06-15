use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use sycamore::reactive::*;

pub fn bench(c: &mut Criterion) {
    c.bench_function("reactivity_signals", |b| {
        let root = create_root(|| {
            b.iter(|| {
                let child_scope = create_child_scope(|| {
                    let state = create_signal(0);

                    for _i in 0..1000 {
                        state.set(state.get() + 1);
                    }
                });

                child_scope.dispose();
            });
        });
        root.dispose();
    });

    c.bench_function("reactivity_effects", |b| {
        let root = create_root(|| {
            b.iter(|| {
                let child_scope = create_child_scope(|| {
                    let state = create_signal(0);

                    create_effect(move || {
                        let double = state.get() * 2;
                        black_box(double);
                    });
                    for _i in 0..1000 {
                        state.set(state.get() + 1);
                    }
                });

                child_scope.dispose();
            });
        });
        root.dispose();
    });

    c.bench_function("reactivity_map_indexed", |b| {
        let root = create_root(|| {
            b.iter(|| {
                let v = create_signal((0..100).collect::<Vec<_>>());
                let mapped = map_indexed(v, |x| x * 2);
                mapped.track();

                v.set((100..200).collect());
                mapped.track();
            });
        });
        root.dispose();
    });

    c.bench_function("reactivity_map_keyed", |b| {
        let root = create_root(|| {
            b.iter(|| {
                let v = create_signal((0..100).collect::<Vec<_>>());
                let mapped = map_keyed(v, |x| x * 2, |x| *x);
                mapped.track();

                v.set((100..200).collect());
                mapped.track();
            });
        });
        root.dispose();
    });

    c.bench_function("reactivity_context_deeply_nested", |b| {
        b.iter_batched(
            || {
                let mut trigger_handle = None;
                let _ = create_root(|| {
                    let trigger = create_signal(());
                    trigger_handle = Some(trigger);
                    let state = create_signal(0i32);
                    provide_context(state);

                    fn create_nested_child_scopes(depth: usize, cb: impl FnOnce()) {
                        if depth == 0 {
                            cb();
                            return;
                        }

                        create_child_scope(|| {
                            provide_context::<i32>(0i32);
                            create_nested_child_scopes(depth - 1, cb);
                        });
                    }

                    create_nested_child_scopes(100, || {
                        create_effect(move || {
                            trigger.track();
                            let state: Signal<i32> = use_context();
                            black_box(state);
                        });
                    });
                });
                trigger_handle.unwrap()
            },
            |trigger| trigger.set(()),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("deep_creation", |b| {
        b.iter(|| {
            let d = create_root(|| {
                let signal = create_signal(0);
                let mut memos = Vec::<ReadSignal<usize>>::new();
                for _ in 0..1000usize {
                    if let Some(prev) = memos.last().copied() {
                        memos.push(create_memo(move || prev.get() + 1));
                    } else {
                        memos.push(create_memo(move || signal.get() + 1));
                    }
                }
            });
            d.dispose();
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().noise_threshold(0.05 /* noisy CI */);
    targets = bench
}
criterion_main!(benches);
