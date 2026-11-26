package veldtest;

import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.shop.server.PayFacts;
import gen.b.shop.server.PayRequest;
import gen.b.shop.server.ShopService;
import gen.b.shop.server.ShopTestHandler;
import gen.b.shop.shop.Receipt;

/**
 * The generated test handler of {@code tests/rules/endpoints/derived.veld.pending} over a stub
 * application that is never reached (every example is a dry run or brings its result). Used by the
 * JDK server ({@link DerivedServe}) and by Spring.
 */
public final class DerivedStubs {
  private DerivedStubs() {}

  /** The test handler over a stub application. */
  static VeldHandler handler() {
    ShopService stub = new ShopService() {
      @Override
      public Receipt pay(PayRequest request, PayFacts facts, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }
    };
    return ShopTestHandler.create(stub);
  }
}
