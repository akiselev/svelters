Feature: Core
    Scenario: get property
        Given module at "core.js"
        When calling function "getProperty" with args ("Object", "type")
        Then return value is "Getter Hello Object"

    Scenario: setting a property actually calls the setter instead of setting JS object
        Given module at "core.js"
        When calling function "setProperty" with args ("Object", "type", "invalid")
        Then return value is "Getter Hello Object"

    Scenario: setter sets the property
        Given module at "core.js"
        When calling function "setProperty" with args ("Object", "type", "newtype")
        Then return value is "Getter newtype"

    Scenario: get superclass property
        Given module at "core.js"
        When calling function "getProperty" with args ("Widget", "type")
        Then return value is "Getter Hello Object"

    Scenario: get superclass property
        Given module at "core.js"
        When calling function "getProperty" with args ("MainWindow", "type")
        Then return value is "Getter Hello Object"

    Scenario: setting a superclass property actually calls the setter instead of setting JS object
        Given module at "core.js"
        When calling function "setProperty" with args ("Widget", "type", "invalid")
        Then return value is "Getter Hello Object"

    Scenario: setter sets the superclass property
        Given module at "core.js"
        When calling function "setProperty" with args ("Widget", "type", "newtype")
        Then return value is "Getter newtype"

    Scenario: get overriden superclass property
        Given module at "core.js"
        When calling function "getProperty" with args ("BoxLayout", "type")
        Then return value is "Overriden"

    Scenario: get readonly class property
        Given module at "core.js"
        When calling function "getProperty" with args ("Widget", "ro")
        Then return value is "Readonly Hello Widget"

    Scenario: get readonly superclass property
        Given module at "core.js"
        When calling function "getProperty" with args ("BoxLayout", "ro")
        Then return value is "Readonly Hello Widget"

    Scenario: get overrriden readonly superclass property
        Given module at "core.js"
        When calling function "getProperty" with args ("Application", "ro")
        Then return value is "Readonly2 Hello Application"
# "Getter Hello Object"