import * as swcHelpers from "@swc/helpers";
var _foo = /*#__PURE__*/ new WeakMap(), _bar = /*#__PURE__*/ new WeakMap();
// @target: es2015
class A {
    constructor(){
        swcHelpers.classPrivateFieldInit(this, _foo, {
            writable: true,
            value: swcHelpers.classPrivateFieldGet(this, _bar)
        }) // Error
        ;
        swcHelpers.classPrivateFieldInit(this, _bar, {
            writable: true,
            value: 3
        });
    }
}
var _foo1 = /*#__PURE__*/ new WeakMap(), _bar1 = /*#__PURE__*/ new WeakSet();
class A2 {
    constructor(){
        swcHelpers.classPrivateMethodInit(this, _bar1);
        swcHelpers.classPrivateFieldInit(this, _foo1, {
            writable: true,
            value: swcHelpers.classPrivateMethodGet(this, _bar1, bar).call(this)
        }) // No Error
        ;
    }
}
function bar() {
    return 3;
}
var _foo2 = /*#__PURE__*/ new WeakMap(), _bar2 = /*#__PURE__*/ new WeakMap();
class A3 {
    constructor(){
        swcHelpers.classPrivateFieldInit(this, _bar2, {
            get: get_bar,
            set: void 0
        });
        swcHelpers.classPrivateFieldInit(this, _foo2, {
            writable: true,
            value: swcHelpers.classPrivateFieldGet(this, _bar2)
        }) // No Error
        ;
    }
}
function get_bar() {
    return 3;
}
var _foo3 = /*#__PURE__*/ new WeakMap(), _bar3 = /*#__PURE__*/ new WeakMap();
class B {
    constructor(){
        swcHelpers.classPrivateFieldInit(this, _foo3, {
            writable: true,
            value: swcHelpers.classPrivateFieldGet(this, _bar3)
        }) // Error
        ;
        swcHelpers.classPrivateFieldInit(this, _bar3, {
            writable: true,
            value: swcHelpers.classPrivateFieldGet(this, _foo3)
        });
    }
}
