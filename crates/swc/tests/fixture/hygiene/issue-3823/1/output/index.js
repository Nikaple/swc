'use strict';
function print() {
    var value$43 = 2;
    var value$41 = value$43 + 100;
    var c = 1;
    if (true) {
        var value$42 = "bbb";
        if (value$42 === "bbb") c = 3;
    }
    [
        0,
        1
    ].map(function(value) {
        var value$4 = 2;
        var x = value ? value$4 : "";
        return x;
    });
    return {
        aaa: value$43,
        bbb: c
    };
}
console.log(print());
