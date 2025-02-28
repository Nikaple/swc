import * as swcHelpers from "@swc/helpers";
// @target: ES5
// @lib: es5,es2015.promise
// @noEmitHelpers: true
class C {
    method() {
        function other() {}
        function fn() {
            return _fn.apply(this, arguments);
        }
        function _fn() {
            _fn = swcHelpers.asyncToGenerator(function*() {
                yield other.apply(this, arguments);
            });
            return _fn.apply(this, arguments);
        }
    }
}
