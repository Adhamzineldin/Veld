package com.nexusbank.transactions.app;

import com.nexusbank.transactions.adapters.JdkHttpServerAdapter;
import com.nexusbank.transactions.runtime.VeldHandler;
import com.nexusbank.transactions.transactions.server.TransactionsHandler;
import com.nexusbank.transactions.transactions.server.TransactionsHooks;
import java.net.InetSocketAddress;

/** Starts the transactions service on the JDK HTTP server adapter (no dependencies). */
public final class TransactionServiceMain {
  private TransactionServiceMain() {}

  public static void main(String[] args) throws Exception {
    int port = Integer.parseInt(System.getenv().getOrDefault("PORT", "3003"));
    String host = System.getenv().getOrDefault("HOST", "127.0.0.1");
    VeldHandler handler =
        TransactionsHandler.create(
            new TransactionsServiceImpl(), new TransactionsHooks(TransactionsServiceImpl::authenticate));
    JdkHttpServerAdapter.listen(handler, new InetSocketAddress(host, port));
    System.out.println("transaction-service listening on port " + port);
  }
}
