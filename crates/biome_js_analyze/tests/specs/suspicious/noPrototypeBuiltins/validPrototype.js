/* should not generate diagnostics */
foo.call(Object.prototype.hasOwnProperty, Object.prototype.hasOwnProperty.call);
Object.prototype;
Object.prototype(obj, prop);
Object.prototype.hasOwnProperty.call;
foo.Object.prototype.hasOwnProperty.call(obj, prop);
foo.prototype.hasOwnProperty.call(obj, prop);
Object.prototype.foo.call(obj, prop);
Object.prototype.hasOwnProperty.foo(obj, prop);
Object.prototype.hasOwnProperty.call.foo(obj, prop);
Object.prototype.prototype.hasOwnProperty.call(a, b);
Object.hasOwnProperty.prototype.hasOwnProperty.call(a, b);
Object.prototype[hasOwnProperty].call(obj, prop);
Object.prototype.hasOwnProperty[call](obj, prop);
Object[prototype].hasOwnProperty.call(obj, prop);
({}).prototype.hasOwnProperty.call(a, b);