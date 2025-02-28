import * as swcHelpers from "@swc/helpers";
import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect } from 'react';
import { select, selectAll } from 'd3-selection';
export default function Home() {
    useEffect(function() {
        new MyClass();
    }, []);
    return /*#__PURE__*/ _jsxs("svg", {
        children: [
            /*#__PURE__*/ _jsxs("g", {
                className: "group",
                children: [
                    /*#__PURE__*/ _jsx("path", {}),
                    /*#__PURE__*/ _jsx("path", {})
                ]
            }),
            /*#__PURE__*/ _jsxs("g", {
                className: "group",
                children: [
                    /*#__PURE__*/ _jsx("path", {}),
                    /*#__PURE__*/ _jsx("path", {})
                ]
            })
        ]
    });
};
var MyClass = function MyClass() {
    "use strict";
    swcHelpers.classCallCheck(this, MyClass);
    selectAll('.group').each(function() {
        select(this).selectAll('path');
    });
};
