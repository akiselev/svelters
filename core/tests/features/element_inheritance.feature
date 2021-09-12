Feature: Core
    Scenario: ELEMENT Inherited nested member is overriden correctly
        Given module at "core.js"
        When calling function "callElementMethod" with args ("mainWindow", "nestedFn")
        Then return value is "NESTED"

    Scenario: ELEMENT Inherited member is not overriden
        Given module at "core.js"
        When calling function "callElementMethod" with args ("widget", "nestedFn")
        Then return value is "Object"

    Scenario: ELEMENT Cast self correctly with caster
        Given module at "core.js"
        When calling function "callElementMethodAfterCast" with args ("object", "Object", "className2")
        Then return value is "Object"

    Scenario: ELEMENT Cast to super correctly with caster
        Given module at "core.js"
        When calling function "callElementMethodAfterCast" with args ("application", "Object", "className2")
        Then return value is "Object"

    Scenario: ELEMENT Cast to nested correctly with caster
        Given module at "core.js"
        When calling function "callElementMethodAfterCast" with args ("mainWindow", "Widget", "className2")
        Then return value is "Widget"

    Scenario: ELEMENT Casted types can call child functions
        Given module at "core.js"
        When calling function "callElementMethodAfterCast" with args (<class>, "Object", "className2")
        Then return value is <result>

        Examples:
            | class        | result   |
            | "widget"     | "Object" |
            | "mainWindow" | "Object" |
            | "boxLayout"  | "Object" |
            | "pushButton" | "Object" |


    Scenario: ELEMENT IMPL self correctly with caster
        Given module at "core.js"
        When calling function "callElementMethodAsImpl" with args ("object", "Object", "className2")
        Then return value is "Object"

    Scenario: ELEMENT IMPL to super correctly with caster
        Given module at "core.js"
        When calling function "callElementMethodAsImpl" with args ("application", "Object", "className2")
        Then return value is "Object"

    Scenario: ELEMENT IMPL to nested correctly with caster
        Given module at "core.js"
        When calling function "callElementMethodAsImpl" with args ("mainWindow", "Widget", "className2")
        Then return value is "Widget"

    Scenario: ELEMENT IMPL types can call child functions
        Given module at "core.js"
        When calling function "callElementMethodAsImpl" with args (<class>, "Object", "className2")
        Then return value is <result>

        Examples:
            | class        | result   |
            | "widget"     | "Object" |
            | "mainWindow" | "Object" |
            | "boxLayout"  | "Object" |
            | "pushButton" | "Object" |

    #
    # NOT USING THESE TESTS FOR NOW BECAUSE NOT SURE IF ELEMENT PROTOTYPE SHOULD HAVE
    #   ACCESS TO CLASS STATIC FUNCTIONS
    #
    # Scenario: ELEMENT check that the Object staticFn is inherited by each child class
    #     Given module at "core.js"
    #     When calling function "callElementStatic" with args (<class>, "staticFn2")
    #     Then return value is <result>

    #     Examples:
    #         | class         | result        |
    #         | "object"      | "__STATIC__"  |
    #         | "application" | "APPLICATION" |
    #         | "widget"      | "WIDGET"      |
    #         | "mainWindow"  | "MAINWINDOW"  |
    #         | "boxLayout"   | "BOXLAYOUT"   |
    #         | "pushButton"  | "PUSHBUTTON"  |

    # Scenario: ELEMENT check that the Object staticFn is overridden by each child class
    #     Given module at "core.js"
    #     When calling function "callElementStatic" with args (<class>, "staticFn")
    #     Then return value is <result>

    #     Examples:
    #         | class         | result       |
    #         | "object"      | "__STATIC__" |
    #         | "application" | "__STATIC__" |
    #         | "widget"      | "__STATIC__" |
    #         | "mainWindow"  | "__STATIC__" |
    #         | "boxLayout"   | "__STATIC__" |
    #         | "pushButton"  | "__STATIC__" |


    Scenario: ELEMENT check that the Object method is inherited by each child class
        Given module at "core.js"
        When calling function "callElementMethod" with args (<class>, "testMethod")
        Then return value is <result>

        Examples:
            | class         | result       |
            | "object"      | "__OBJECT__" |
            | "application" | "__OBJECT__" |
            | "widget"      | "__OBJECT__" |
            | "mainWindow"  | "__OBJECT__" |
            | "boxLayout"   | "__OBJECT__" |
            | "pushButton"  | "__OBJECT__" |

    Scenario: ELEMENT check that the className getter is overriden by each child class
        Given module at "core.js"
        When calling function "getElementClassName" with args (<class>)
        Then return value is <result>

        Examples:
            | class         | result        |
            | "object"      | "Object"      |
            | "application" | "Application" |
            | "widget"      | "Widget"      |
            | "mainWindow"  | "MainWindow"  |
            | "boxLayout"   | "BoxLayout"   |
            | "pushButton"  | "PushButton"  |

    Scenario: ELEMENT check that methods are overriden by each child class (generic)
        Given module at "core.js"
        When calling function "callElementMethod" with args (<class>, "className2")
        Then return value is <result>

        Examples:
            | class         | result           |
            | "object"      | "Object"         |
            | "application" | "Application"    |
            | "widget"      | "Widget"         |
            | "mainWindow"  | "MainWindow"     |
            | "boxLayout"   | "Widget"         |
            | "pushButton"  | "AbstractButton" |
