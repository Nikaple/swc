import * as swcHelpers from "@swc/helpers";
var C = /*#__PURE__*/ function() {
    "use strict";
    function C() {
        swcHelpers.classCallCheck(this, C);
    }
    var _proto = C.prototype;
    _proto.foo = function foo() {};
    C.foo = function foo() {};
    swcHelpers.createClass(C, [
        {
            key: "y",
            get: function get() {
                return null;
            },
            set: function set(x) {}
        }
    ], [
        {
            key: "b",
            get: function get() {
                return null;
            },
            set: function set(x) {}
        }
    ]);
    return C;
}();
var c;
c.x;
c.y;
c.y = 1;
c.foo();
C.a;
C.b();
C.b = 1;
C.foo();
