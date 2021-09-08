Feature: Core
    Scenario: Inherited nested member is overriden correctly
        Given module at "core.js"
        When calling function "callMethod" with args ("MainWindow", "nestedFn")
        Then return value is "NESTED"

    Scenario: Inherited member is not overriden
        Given module at "core.js"
        When calling function "callMethod" with args ("Widget", "nestedFn")
        Then return value is "Object"

    Scenario: Cast self correctly with caster
        Given module at "core.js"
        When calling function "callMethodAfterCast" with args ("Object", "Object", "className2")
        Then return value is "Object"

    Scenario: Cast to super correctly with caster
        Given module at "core.js"
        When calling function "callMethodAfterCast" with args ("Application", "Object", "className2")
        Then return value is "Object"

    Scenario: Cast to nested correctly with caster
        Given module at "core.js"
        When calling function "callMethodAfterCast" with args ("MainWindow", "Widget", "className2")
        Then return value is "Widget"

    Scenario: Casted types can call child functions
        Given module at "core.js"
        When calling function "callMethodAfterCast" with args (<class>, "Object", "className2")
        Then return value is <result>

        Examples:
            | class        | result   |
            | "Widget"     | "Object" |
            | "MainWindow" | "Object" |
            | "BoxLayout"  | "Object" |
            | "PushButton" | "Object" |

    Scenario: check that the Object staticFn is inherited by each child class
        Given module at "core.js"
        When calling function "callStatic" with args (<class>, "staticFn2")
        Then return value is <result>

        Examples:
            | class         | result        |
            | "Object"      | "__STATIC__"  |
            | "Application" | "APPLICATION" |
            | "Widget"      | "WIDGET"      |
            | "MainWindow"  | "MAINWINDOW"  |
            | "BoxLayout"   | "BOXLAYOUT"   |
            | "PushButton"  | "PUSHBUTTON"  |

    Scenario: check that the Object staticFn is overridden by each child class
        Given module at "core.js"
        When calling function "callStatic" with args (<class>, "staticFn")
        Then return value is <result>

        Examples:
            | class         | result       |
            | "Object"      | "__STATIC__" |
            | "Application" | "__STATIC__" |
            | "Widget"      | "__STATIC__" |
            | "MainWindow"  | "__STATIC__" |
            | "BoxLayout"   | "__STATIC__" |
            | "PushButton"  | "__STATIC__" |


    Scenario: check that the Object method is inherited by each child class
        Given module at "core.js"
        When calling function "callMethod" with args (<class>, "testMethod")
        Then return value is <result>

        Examples:
            | class         | result       |
            | "Object"      | "__OBJECT__" |
            | "Application" | "__OBJECT__" |
            | "Widget"      | "__OBJECT__" |
            | "MainWindow"  | "__OBJECT__" |
            | "BoxLayout"   | "__OBJECT__" |
            | "PushButton"  | "__OBJECT__" |

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

    Scenario: check that Virt works
        Given module at "core.js"
        When calling function "addWidget" with args (<class>)
        Then return value is <result>

        Examples:
            | class        | result              |
            | "MainWindow" | "ADD_WIDGET_RESULT" |
            | "BoxLayout"  | "ADD_WIDGET_RESULT" |
            | "PushButton" | "ADD_WIDGET_RESULT" |

    Scenario: check that Virt obj works
        Given module at "core.js"
        When calling function "addObject" with args (<class>)
        Then return value is <result>

        Examples:
            | class         | result           |
            | "Application" | "Application"    |
            | "Widget"      | "Widget"         |
            | "MainWindow"  | "MainWindow"     |
            | "BoxLayout"   | "Widget"         |
            | "PushButton"  | "AbstractButton" |