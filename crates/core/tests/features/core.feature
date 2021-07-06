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


    Scenario: check that the className getter is overriden by each child class
        Given module at "core.js"
        When calling function "getClassName" with args (<class>)
        Then return value is <result>

        Examples:
            | class         | result        |
            | "Object"      | "Object"      |
            | "Application" | "Application" |
            | "Widget"      | "Widget"      |
            | "MainWindow"  | "MainWindow"  |
            | "BoxLayout"   | "BoxLayout"   |
            | "PushButton"  | "PushButton"  |

    Scenario: check that methods are overriden by each child class (generic)
        Given module at "core.js"
        When calling function "callMethod" with args (<class>, "className2")
        Then return value is <result>

        Examples:
            | class         | result           |
            | "Object"      | "Object"         |
            | "Application" | "Application"    |
            | "Widget"      | "Widget"         |
            | "MainWindow"  | "MainWindow"     |
            | "BoxLayout"   | "Widget"         |
            | "PushButton"  | "AbstractButton" |

    Scenario: check Object constructor
        Given module at "core.js"
        When calling function "constructClass" with args ("Object")
        Then return value is object