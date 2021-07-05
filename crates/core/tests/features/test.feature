Feature: Core

    Scenario: testing runner
        Given module at "test.js"
        When calling function "testConsole"

    Scenario: testing return value
        Given module at "test.js"
        When calling function "testReturnValue" with args ("Runner")
        Then return value is "Hello Runner"

    Scenario: testing args
        Given module at "test.js"
        When calling function "testArgs" with args ("One", "Two", "Three", "Four", "Five")
        Then return value is "One Two Three Four Five"
