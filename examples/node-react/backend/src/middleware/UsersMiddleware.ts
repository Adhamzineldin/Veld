import {IUsersMiddleware} from "@veld/generated/middleware/IUsersMiddleware";


export class UsersMiddleware implements IUsersMiddleware {
    validateCreateUserInput(req: any, res: any, next: () => void): void {
        console.log("Validating create user input");
    }
}

