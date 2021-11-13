import { TransformationCase } from "./transformation-case";

export interface Transformations {
    [type: string]: {
        transformation: TransformationCase;
    };
}
