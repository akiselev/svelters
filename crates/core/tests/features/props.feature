Feature: Core
    # Scenario: get property
    #     Given module at "core.js"
    #     When calling function "getProperty" with args ("Object", "type")
    #     Then return value is "Object"

    # Scenario: setting a property actually calls the setter instead of setting JS object
    #     Given module at "core.js"
    #     When calling function "setProperty" with args ("Object", "type", "invalid")
    #     Then return value is "Object"

    Scenario: setter sets the property
        Given module at "core.js"
        When calling function "setProperty" with args ("Object", "type", "newtype")
        Then return value is "newtype"