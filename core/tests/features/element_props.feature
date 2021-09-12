Feature: Core
    Scenario: get Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("object", "type")
        Then return value is "Getter Hello Object"

    Scenario: setting a Element property actually calls the setter instead of setting JS object
        Given module at "core.js"
        When calling function "setElementProperty" with args ("object", "type", "invalid")
        Then return value is "Getter Hello Object"

    Scenario: setter sets the Element property
        Given module at "core.js"
        When calling function "setElementProperty" with args ("object", "type", "newtype")
        Then return value is "Getter newtype"

    Scenario: get superclass Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("widget", "type")
        Then return value is "Getter Hello Object"

    Scenario: get superclass Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("mainWindow", "type")
        Then return value is "Getter Hello Object"

    Scenario: setting a superclass Element property actually calls the setter instead of setting JS object
        Given module at "core.js"
        When calling function "setElementProperty" with args ("widget", "type", "invalid")
        Then return value is "Getter Hello Object"

    Scenario: setter sets the superclass Element property
        Given module at "core.js"
        When calling function "setElementProperty" with args ("widget", "type", "newtype")
        Then return value is "Getter newtype"

    Scenario: get overriden superclass Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("boxLayout", "type")
        Then return value is "Overriden"

    Scenario: get readonly class Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("widget", "ro")
        Then return value is "Readonly Hello Widget"

    Scenario: get readonly superclass Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("boxLayout", "ro")
        Then return value is "Readonly Hello Widget"

    Scenario: get overrriden readonly superclass Element property
        Given module at "core.js"
        When calling function "getElementProperty" with args ("application", "ro")
        Then return value is "Readonly2 Hello Application"