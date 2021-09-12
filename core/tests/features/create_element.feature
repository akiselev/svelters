Feature: Core
    Scenario: Create element
        Given module at "core.js"
        When calling function "callElementMethod" with args ("mainWindow")
        Then return value is "MainWindow"

    Scenario: Create element in bulk
        Given module at "core.js"
        When calling function "callElementMethod" with args (<class>, "className2")
        Then return value is <result>

        Examples:
            | class        | result           |
            | "mainWindow" | "MainWindow"     |
            | "boxLayout"  | "Widget"         |
            | "pushButton" | "AbstractButton" |

    Scenario: ELEMENT Get property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("object", "type")
        Then return value is "Getter Hello Object"

    Scenario: ELEMENT Setting a property actually calls the setter instead of setting JS object
        Given module at "core.js"
        When calling function "setElementProperty" with args ("object", "type", "invalid")
        Then return value is "Getter Hello Object"

    Scenario: ELEMENT Setter sets the property
        Given module at "core.js"
        When calling function "setElementProperty" with args ("object", "type", "newtype")
        Then return value is "Getter newtype"

    Scenario: ELEMENT Get superclass property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("widget", "type")
        Then return value is "Getter Hello Object"

    Scenario: ELEMENT Get superclass property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("mainWindow", "type")
        Then return value is "Getter Hello Object"

    Scenario: ELEMENT Setting a superclass property actually calls the setter instead of setting JS object
        Given module at "core.js"
        When calling function "setElementProperty" with args ("widget", "type", "invalid")
        Then return value is "Getter Hello Object"

    Scenario: ELEMENT Setter sets the superclass property
        Given module at "core.js"
        When calling function "setElementProperty" with args ("widget", "type", "newtype")
        Then return value is "Getter newtype"

    Scenario: ELEMENT Get overriden superclass property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("boxLayout", "type")
        Then return value is "Overriden"

    Scenario: ELEMENT Get readonly class property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("widget", "ro")
        Then return value is "Readonly Hello Widget"

    Scenario: ELEMENT Get readonly superclass property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("boxLayout", "ro")
        Then return value is "Readonly Hello Widget"

    Scenario: ELEMENT Get overrriden readonly superclass property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("application", "ro")
        Then return value is "Readonly2 Hello Application"