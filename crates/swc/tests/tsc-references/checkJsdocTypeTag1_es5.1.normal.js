// @allowJS: true
// @suppressOutputPathCheck: true
// @filename: 0.js
// @ts-check
/** @type {String} */ var S = "hello world";
/** @type {number} */ var n = 10;
/** @type {*} */ var anyT = 2;
anyT = "hello";
/** @type {?} */ var anyT1 = 2;
anyT1 = "hi";
/** @type {Function} */ var x = function(a) {
    return a + 1;
};
x(1);
/** @type {function} */ var y = function(a) {
    return a + 1;
};
y(1);
/** @type {function (number)} */ var x1 = function(a) {
    return a + 1;
};
x1(0);
/** @type {function (number): number} */ var x2 = function(a) {
    return a + 1;
};
x2(0);
/**
 * @type {object}
 */ var props = {};
/**
 * @type {Object}
 */ var props = {};
