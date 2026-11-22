package veldtest;

import eqj.runtime.ClientOptions;
import eqj.runtime.VeldError;
import eqj.shop.client.AlphaClient;
import eqj.shop.client.AlphaFetchRequest;
import eqj.shop.client.BetaClient;
import eqj.shop.client.BetaFetchRequest;
import eqj.shop.shop.Gone;

/**
 * Calls the Node servers through the generated Java client and prints what decoding the declared
 * error yields, one line per call: {@code service id class sig op canonical}. The TypeScript client
 * prints the same lines (tests/toolchain/ts/eq_decode.ts); the test requires them to be identical.
 * Usage: {@code EqDecode <alpha url> <beta url>}.
 */
public final class EqDecode {
  private EqDecode() {}

  private static void line(String service, String id, VeldError e) {
    String cls = e instanceof Gone ? "Gone" : e.getClass().getSimpleName();
    System.out.println(String.join("\t", service, id, cls, e.sig(), e.op(), e.veldCanonical()));
  }

  public static void main(String[] args) {
    AlphaClient alpha = new AlphaClient(ClientOptions.builder().baseUrl(args[0]).build());
    BetaClient beta = new BetaClient(ClientOptions.builder().baseUrl(args[1]).build());
    for (String id : new String[] {"x", "y"}) {
      try {
        alpha.fetch(new AlphaFetchRequest(id));
        throw new IllegalStateException("Alpha.fetch must fail");
      } catch (VeldError e) {
        line("Alpha", id, e);
      }
      try {
        beta.fetch(new BetaFetchRequest(id));
        throw new IllegalStateException("Beta.fetch must fail");
      } catch (VeldError e) {
        line("Beta", id, e);
      }
    }
  }
}
