import {IMiddleware} from "@veld/generated/middleware/IMiddleware";


export class UsersMiddleware implements IMiddleware {
    validateCreateUserInput(req: any, res: any, next: () => void): void {
        console.log("Validating create user input");
        next();
    }
}

