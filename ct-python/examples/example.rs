use ct_python::ct_python;

ct_python! {
	print("static A: i32 = 1;")
}

static DIRECTIONS: [(f64, f64); 4] = ct_python! {
	from math import sin, cos, tau
	n = 4
	print("[")
	for i in range(n):
		print("(", cos(i / n * tau), ",", sin(i / n * tau), "),")
	print("]")
};

fn main() {
	dbg!(&A);
	dbg!(&DIRECTIONS);
}
