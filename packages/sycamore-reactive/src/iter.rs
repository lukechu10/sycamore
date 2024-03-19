//! Reactive utilities for dealing with lists and iterables.

use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

use crate::*;

/// Function that maps a `Vec` to another `Vec` via a map function. The mapped `Vec` is lazy
/// computed, meaning that it's value will only be updated when requested. Modifications to the
/// input `Vec` are diffed using keys to prevent recomputing values that have not changed.
///
/// This function is the underlying utility behind `Keyed`.
///
/// # Params
/// * `list` - The list to be mapped. The list must be a [`ReadSignal`] (obtained from a [`Signal`])
///   and therefore reactive.
/// * `map_fn` - A closure that maps from the input type to the output type.
/// * `key_fn` - A closure that returns an _unique_ key to each entry.
///
///  _Credits: Based on TypeScript implementation in <https://github.com/solidjs/solid>_
pub fn map_keyed<T, K, U: 'static>(
    list: impl Accessor<Vec<T>> + 'static,
    mut map_fn: impl FnMut(T) -> U + 'static,
    key_fn: impl Fn(&T) -> K + 'static,
) -> ReadSignal<Vec<U>>
where
    T: PartialEq + Clone + 'static,
    K: Eq + Hash,
    U: Clone,
{
    // Previous state used for diffing.
    let mut items = Vec::new();

    let mut mapped: Vec<U> = Vec::new();
    let mut mapped_tmp: Vec<Option<U>> = Vec::new();

    let mut disposers: Vec<Option<NodeHandle>> = Vec::new();
    let mut disposers_tmp: Vec<Option<NodeHandle>> = Vec::new();

    // Diff and update signal each time list is updated.
    let mut update = move || {
        let new_items = list.value();
        if new_items.is_empty() {
            // Fast path for removing all items.
            for dis in mem::take(&mut disposers) {
                dis.unwrap().dispose();
            }
            mapped = Vec::new();
        } else if items.is_empty() {
            // Fast path for new create.
            mapped.reserve(new_items.len());
            disposers.reserve(new_items.len());

            for new_item in new_items.iter().cloned() {
                let map_fn = &mut map_fn;
                let mapped = &mut mapped;
                let new_disposer = create_child_scope(move || mapped.push(map_fn(new_item)));
                disposers.push(Some(new_disposer));
            }
        } else {
            mapped_tmp.clear();
            mapped_tmp.resize(new_items.len(), None);

            disposers_tmp.clear();
            disposers_tmp.resize_with(new_items.len(), || None);

            // Skip common prefix.
            let min_len = usize::min(items.len(), new_items.len());
            let start = items
                .iter()
                .zip(new_items.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(min_len);
            debug_assert!(
                (items.get(start).is_none() && new_items.get(start).is_none())
                    || (items.get(start) != new_items.get(start)),
                "start is the first index where items[start] != new_items[start]"
            );

            // Skip common suffix.
            let mut end = items.len();
            let mut new_end = new_items.len();
            while end > start && new_end > start && items[end - 1] == new_items[new_end - 1] {
                end -= 1;
                new_end -= 1;
                mapped_tmp[new_end] = Some(mapped[end].clone());
                disposers_tmp[new_end] = disposers[end].take();
            }
            debug_assert!(
                    if end != 0 && new_end != 0 {
                        (end == items.len() && new_end == new_items.len())
                            || (items[end - 1] != new_items[new_end - 1])
                    } else {
                        true
                    },
                    "end and new_end are the last indexes where items[end - 1] != new_items[new_end - 1]"
                );

            // 0) Prepare a map of indices in new_items. Scan backwards so we encounter them in
            // natural order.
            let mut new_indices = HashMap::with_capacity(new_end - start);

            // Indexes for new_indices_next are shifted by start because values at 0..start are
            // always None.
            let mut new_indices_next = vec![None; new_end - start];
            for j in (start..new_end).rev() {
                let item = &new_items[j];
                let key = key_fn(item);
                let i = new_indices.get(&key);
                new_indices_next[j - start] = i.copied();
                new_indices.insert(key, j);
            }

            // 1) Step through old items and see if they can be found in new set; if so, mark
            // them as moved.
            for i in start..end {
                let item = &items[i];
                let key = key_fn(item);
                if let Some(j) = new_indices.get(&key).copied() {
                    // Moved. j is index of item in new_items.
                    mapped_tmp[j] = Some(mapped[i].clone());
                    disposers_tmp[j] = disposers[i].take();
                    new_indices_next[j - start].and_then(|j| new_indices.insert(key, j));
                } else {
                    // Create new.
                    disposers[i].take().unwrap().dispose();
                }
            }

            // 2) Set all the new values, pulling from the moved array if copied, otherwise
            // entering the new value.
            for j in start..new_items.len() {
                if matches!(mapped_tmp.get(j), Some(Some(_))) {
                    // Pull from moved array.
                    if j >= mapped.len() {
                        debug_assert_eq!(mapped.len(), j);
                        mapped.push(mapped_tmp[j].clone().unwrap());
                        disposers.push(disposers_tmp[j].take());
                    } else {
                        mapped[j] = mapped_tmp[j].clone().unwrap();
                        disposers[j] = disposers_tmp[j].take();
                    }
                } else {
                    // Create new value.
                    let mut tmp = None;
                    let new_item = new_items[j].clone();
                    let new_disposer = create_child_scope(|| tmp = Some(map_fn(new_item)));
                    if mapped.len() > j {
                        mapped[j] = tmp.unwrap();
                        disposers[j] = Some(new_disposer);
                    } else {
                        mapped.push(tmp.unwrap());
                        disposers.push(Some(new_disposer));
                    }
                }
            }
        }

        // 3) In case the new set is shorter than the old, set the length of the mapped array.
        mapped.truncate(new_items.len());
        disposers.truncate(new_items.len());

        // 4) Save a copy of the mapped items for the next update.
        debug_assert!([mapped.len(), disposers.len()]
            .iter()
            .all(|l| *l == new_items.len()));
        items = new_items;

        mapped.clone()
    };
    let scope = use_current_scope();
    create_memo(move || scope.run_in(&mut update))
}

/// Function that maps a `Vec` to another `Vec` via a map function. The mapped `Vec` is lazy
/// computed, meaning that it's value will only be updated when requested. Modifications to the
/// input `Vec` are diffed by index to prevent recomputing values that have not changed.
///
/// Generally, it is preferred to use [`map_keyed`] instead when a key function
/// is available.
///
/// This function is the underlying utility behind `Indexed`.
///
/// # Params
/// * `list` - The list to be mapped. The list must be a [`ReadSignal`] (obtained from a [`Signal`])
///   and therefore reactive.
/// * `map_fn` - A closure that maps from the input type to the output type.
pub fn map_indexed<T, U: 'static>(
    list: impl Accessor<Vec<T>> + 'static,
    mut map_fn: impl FnMut(T) -> U + 'static,
) -> ReadSignal<Vec<U>>
where
    T: PartialEq + Clone + 'static,
    U: Clone,
{
    // Previous state used for diffing.
    let mut items = Vec::new();
    let mut mapped = Vec::new();
    let mut disposers: Vec<NodeHandle> = Vec::new();

    // Diff and update signal each time list is updated.
    let mut update = move || {
        let new_items = list.value();

        if new_items.is_empty() {
            // Fast path for removing all items.
            for dis in mem::take(&mut disposers) {
                dis.dispose();
            }
            items = Vec::new();
            mapped = Vec::new();
        } else {
            // Pre-allocate space needed
            if new_items.len() > items.len() {
                let new_count = new_items.len() - items.len();
                mapped.reserve(new_count);
                disposers.reserve(new_count);
            }

            for (i, new_item) in new_items.iter().cloned().enumerate() {
                let item = items.get(i);
                // We lift the equality out of the else if branch to satisfy borrow checker.
                let eqs = item != Some(&new_item);

                if item.is_none() || eqs {
                    let mut tmp = None;
                    let new_disposer = create_child_scope(|| tmp = Some(map_fn(new_item)));
                    if item.is_none() {
                        mapped.push(tmp.unwrap());
                        disposers.push(new_disposer);
                    } else if eqs {
                        mapped[i] = tmp.unwrap();
                        let prev = mem::replace(&mut disposers[i], new_disposer);
                        prev.dispose();
                    }
                }
            }

            if new_items.len() < items.len() {
                for _i in new_items.len()..items.len() {
                    disposers.pop().unwrap().dispose();
                }
            }

            // In case the new set is shorter than the old, set the length of the mapped array.
            mapped.truncate(new_items.len());

            // Save a copy of the mapped items for the next update.
            debug_assert!([mapped.len(), disposers.len()]
                .iter()
                .all(|l| *l == new_items.len()));
            items = new_items;
        }

        // Update signal to trigger updates.
        mapped.clone()
    };
    let scope = use_current_scope();
    create_memo(move || scope.run_in(&mut update))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn keyed() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let mapped = map_keyed(a, |x| x * 2, |x| *x);
            assert_eq!(mapped.get_clone(), vec![2, 4, 6]);

            a.set(vec![1, 2, 3, 4]);
            assert_eq!(mapped.get_clone(), vec![2, 4, 6, 8]);

            a.set(vec![2, 2, 3, 4]);
            assert_eq!(mapped.get_clone(), vec![4, 4, 6, 8]);
        });
    }

    #[test]
    fn keyed_recompute_everything() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let mapped = map_keyed(a, |x| x * 2, |x| *x);
            assert_eq!(mapped.get_clone(), vec![2, 4, 6]);

            a.set(vec![4, 5, 6]);
            assert_eq!(mapped.get_clone(), vec![8, 10, 12]);
        });
    }

    /// Test fast path for clearing Vec.
    #[test]
    fn keyed_clear() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let mapped = map_keyed(a, |x| x * 2, |x| *x);

            a.set(Vec::new());
            assert_eq!(mapped.get_clone(), Vec::<i32>::new());
        });
    }

    /// Test that using [`Scope::map_keyed`] will reuse previous computations.
    #[test]
    fn keyed_use_previous_computation() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let counter = Rc::new(Cell::new(0));
            let mapped = map_keyed(
                a,
                {
                    let counter = Rc::clone(&counter);
                    move |_| {
                        counter.set(counter.get() + 1);
                        counter.get()
                    }
                },
                |x| *x,
            );
            assert_eq!(mapped.get_clone(), vec![1, 2, 3]);

            a.set(vec![1, 2]);
            assert_eq!(mapped.get_clone(), vec![1, 2]);

            a.set(vec![1, 2, 4]);
            assert_eq!(mapped.get_clone(), vec![1, 2, 4]);

            a.set(vec![1, 2, 3, 4]);
            assert_eq!(mapped.get_clone(), vec![1, 2, 5, 4]);
        });
    }

    #[test]
    fn keyed_call_cleanup_on_remove() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let counter = Rc::new(Cell::new(0));
            let _mapped = map_keyed(
                a,
                {
                    let counter = Rc::clone(&counter);
                    move |_| {
                        let counter = Rc::clone(&counter);
                        on_cleanup(move || {
                            counter.set(counter.get() + 1);
                        });
                    }
                },
                |x| *x,
            );
            assert_eq!(counter.get(), 0, "no cleanup yet");

            a.set(vec![1, 2]);
            assert_eq!(counter.get(), 1);

            a.set(vec![1, 2, 3]);
            assert_eq!(counter.get(), 1);

            a.set(vec![1, 3]);
            assert_eq!(counter.get(), 2);
        });
    }

    #[test]
    fn keyed_call_cleanup_on_remove_all() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let counter = Rc::new(Cell::new(0));
            let _mapped = map_keyed(
                a,
                {
                    let counter = Rc::clone(&counter);
                    move |_| {
                        let counter = Rc::clone(&counter);
                        on_cleanup(move || {
                            counter.set(counter.get() + 1);
                        })
                    }
                },
                |x| *x,
            );
            assert_eq!(counter.get(), 0, "no cleanup yet");

            a.set(vec![]);
            assert_eq!(counter.get(), 3);
        });
    }

    #[test]
    fn indexed() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let mapped = map_indexed(a, |x| x * 2);
            assert_eq!(mapped.get_clone(), vec![2, 4, 6]);

            a.set(vec![1, 2, 3, 4]);
            assert_eq!(mapped.get_clone(), vec![2, 4, 6, 8]);

            a.set(vec![2, 2, 3, 4]);
            assert_eq!(mapped.get_clone(), vec![4, 4, 6, 8]);
        });
    }

    /// Test fast path for clearing Vec.
    #[test]
    fn indexed_clear() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let mapped = map_indexed(a, |x| x * 2);

            a.set(Vec::new());
            assert_eq!(mapped.get_clone(), Vec::<i32>::new());
        });
    }

    /// Test that result of mapped function can be listened to.
    #[test]
    fn indexed_react() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let mapped = map_indexed(a, |x| x * 2);

            let counter = create_signal(0);
            create_effect(move || {
                counter.set(counter.get_untracked() + 1);
                mapped.track();
            });

            assert_eq!(counter.get(), 1);
            a.set(vec![1, 2, 3, 4]);
            assert_eq!(counter.get(), 2);
        });
    }

    /// Test that using [`map_indexed`] will reuse previous computations.
    #[test]
    fn indexed_use_previous_computation() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let counter = Rc::new(Cell::new(0));
            let mapped = map_indexed(a, {
                let counter = Rc::clone(&counter);
                move |_| {
                    counter.set(counter.get() + 1);
                    counter.get()
                }
            });
            assert_eq!(mapped.get_clone(), vec![1, 2, 3]);

            a.set(vec![1, 2]);
            assert_eq!(mapped.get_clone(), vec![1, 2]);

            a.set(vec![1, 2, 4]);
            assert_eq!(mapped.get_clone(), vec![1, 2, 4]);

            a.set(vec![1, 3, 4]);
            assert_eq!(mapped.get_clone(), vec![1, 5, 4]);
        });
    }

    #[test]
    fn indexed_call_cleanup_on_remove() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let counter = Rc::new(Cell::new(0));
            let _mapped = map_indexed(a, {
                let counter = Rc::clone(&counter);
                move |_| {
                    let counter = Rc::clone(&counter);
                    on_cleanup(move || {
                        counter.set(counter.get() + 1);
                    });
                }
            });
            assert_eq!(counter.get(), 0, "no cleanup yet");

            a.set(vec![1, 2]);
            assert_eq!(counter.get(), 1);

            a.set(vec![1, 2, 3]);
            assert_eq!(counter.get(), 1);

            a.set(vec![1, 3]);
            assert_eq!(counter.get(), 3);
        });
    }

    #[test]
    fn indexed_call_cleanup_on_remove_all() {
        let _ = create_root(|| {
            let a = create_signal(vec![1, 2, 3]);
            let counter = Rc::new(Cell::new(0));
            let _mapped = map_indexed(a, {
                let counter = Rc::clone(&counter);
                move |_| {
                    let counter = Rc::clone(&counter);
                    on_cleanup(move || {
                        counter.set(counter.get() + 1);
                    })
                }
            });
            assert_eq!(counter.get(), 0, "no cleanup yet");

            a.set(vec![]);
            assert_eq!(counter.get(), 3);
        });
    }
}
