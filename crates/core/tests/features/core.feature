Feature: Core
    Scenario: check that Object constructor is a function
        Given module at "core.js"
        When calling function "getTypeOf" with args ("Object")
        Then return value is "function"

    Scenario: check that fake prop is undefined
        Given module at "core.js"
        When calling function "getTypeOf" with args ("Object", "notReal")
        Then return value is "undefined"

    Scenario: check get class name
        Given module at "core.js"
        When calling function "getClassName"
        Then return value is "Object"

    Scenario: check basic method call
        Given module at "core.js"
        When calling function "callMethod" with args ("Object", "className")
        Then return value is "Object"

    Scenario: check has method
        Given module at "core.js"
        When calling function "hasMethod" with args ("Object", "className")
        Then return value is true

    Scenario: check basic static call
        Given module at "core.js"
        When calling function "callStatic" with args ("Object", "staticFn")
        Then return value is "__STATIC__"

    Scenario: check has static
        Given module at "core.js"
        When calling function "hasStatic" with args ("Object", "staticFn")
        Then return value is true

    Scenario: check Object constructor
        Given module at "core.js"
        When calling function "constructClass" with args ("Object")
        Then return value is object