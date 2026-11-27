package com.nexusbank.transactions.app;

import com.nexusbank.transactions.adapters.JdkHttpServerAdapter;
import com.nexusbank.transactions.runtime.VeldHandler;
import com.nexusbank.transactions.runtimeconfig.TransactionsApiRuntime;
import com.nexusbank.transactions.transactions.server.TransactionsHandler;
import com.nexusbank.transactions.transactions.server.TransactionsHooks;
import java.net.InetSocketAddress;
import java.util.Map;

/** Starts the transactions service on the JDK HTTP server adapter (no dependencies). */
public final class TransactionServiceMain {
  private TransactionServiceMain() {}

  public static void main(String[] args) throws Exception {
    int port = Integer.parseInt(System.getenv().getOrDefault("PORT", "3003"));
    String host = System.getenv().getOrDefault("HOST", "127.0.0.1");
    // The URLs of the services this one calls come from the environment, named here and nowhere else.
    TransactionsApiRuntime runtime =
        TransactionsApiRuntime.fromEnv(
            Map.of(
                "services.accounts.url", "VELD_ACCOUNTS_URL",
                "services.iam.url", "VELD_IAM_URL"));
    TransactionsServiceImpl service = new TransactionsServiceImpl(runtime);
    VeldHandler handler =
        TransactionsHandler.create(
            service, new TransactionsHooks(service::authenticate), runtime);
    JdkHttpServerAdapter.listen(handler, new InetSocketAddress(host, port));
    System.out.println("transaction-service listening on port " + port);
  }
}
