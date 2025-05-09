use inline_python::{Context, python};

fn main() {
    python! {
        print("Hello")
    }

    let c = Context::new();

    c.run(python! {
        a = "asdf"
    });

    c.run(python! {
        print(a)
    });

    let result: Context = python! {
        foo = 123 + 4
    };

    result.run(python! {
        foo += 10
    });

    let x: i32 = result.get("foo");

    assert_eq!(x, 137);

    python! {
        print('x)
    }
}
