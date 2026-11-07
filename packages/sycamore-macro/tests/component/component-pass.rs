use sycamore::prelude::{View, component};

#[component]
fn CompNoProps() -> View {
    todo!();
}

#[component]
fn CompWithProps(prop: i32) -> View {
    let _ = prop;
    todo!();
}

#[component]
async fn AsyncCompNoProps() -> View {
    todo!();
}

#[component]
async fn AsyncCompWithProps(prop: i32) -> View {
    let _ = prop;
    todo!();
}

fn main() {}
