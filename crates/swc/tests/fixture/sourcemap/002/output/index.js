import * as swcHelpers from "@swc/helpers";
import regeneratorRuntime from "regenerator-runtime";
export default function StaticPage(param) {
    var data = param.data;
    return /*#__PURE__*/ React.createElement("div", null, data.foo);
};
export function getStaticProps() {
    return _getStaticProps.apply(this, arguments);
}
function _getStaticProps() {
    _getStaticProps = swcHelpers.asyncToGenerator(regeneratorRuntime.mark(function _callee() {
        return regeneratorRuntime.wrap(function _callee$(_ctx) {
            while(1)switch(_ctx.prev = _ctx.next){
                case 0:
                    return _ctx.abrupt("return", {
                        props: {
                            data: {
                                foo: 'bar'
                            }
                        }
                    });
                case 1:
                case "end":
                    return _ctx.stop();
            }
        }, _callee);
    }));
    return _getStaticProps.apply(this, arguments);
}
