// Two call or construct signatures are considered identical when they have the same number of type parameters and, considering those 
// parameters pairwise identical, have identical type parameter constraints, identical number of parameters with identical kind(required, 
// optional or rest) and types, and identical return types.
class One {
}
class Two {
}
class A {
    foo(x, y) {
        return null;
    }
}
class B {
    foo(x, y) {
        return null;
    }
}
class C {
    foo(x, y) {
        return null;
    }
}
class D {
    foo(x, y) {
        return null;
    }
}
var a;
var b = {
    foo (x, y) {
        return '';
    }
};
function foo1(x) {}
function foo1b(x) {}
function foo1c(x) {}
function foo2(x) {}
function foo3(x) {}
function foo4(x) {}
function foo5(x) {}
function foo5b(x) {}
function foo5c(x) {}
function foo6c(x) {}
function foo6(x) {}
function foo7(x) {}
function foo8(x) {}
function foo9(x) {}
function foo10(x) {}
function foo11(x) {}
function foo12(x) {}
function foo12b(x) {}
function foo13(x) {}
function foo14(x) {}
function foo15(x) {}
