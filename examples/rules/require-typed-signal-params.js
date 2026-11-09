/** @type {import("gdguard").Rule} */
export default {
  meta: {id:'require-typed-signal-params',description:'Declare types for signal parameters.',rationale:'Signal parameter types document the callback contract.'},
  create(context) {return {signal_statement(node) {
    for (const parameter of node.field('parameters')?.namedChildren ?? []) {
      if (!parameter.field('type')) context.report({node:parameter,message:'Signal parameter needs a type',help:'Add an explicit type such as : int.'});
    }
  }};},
  tests: {valid:['signal changed(value: int)\n','signal changed\n'],invalid:[{code:'signal changed(value)\n',errors:1}]}
};
