import * as swcHelpers from "@swc/helpers";
var _ref, _method = new WeakSet();
class C {
    constructor(){
        swcHelpers.classPrivateMethodInit(this, _method);
    }
}
C.s = swcHelpers.classPrivateMethodGet(_ref = new C(), _method, function() {
    return 42;
}).call(_ref), console.log(C.s);
