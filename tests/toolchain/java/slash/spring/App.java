package veldtest;

import gen.sl.shop.items.Item;
import gen.sl.shop.server.CountRequest;
import gen.sl.shop.server.CreateRequest;
import gen.sl.shop.server.ItemsHooks;
import gen.sl.shop.server.ItemsService;
import gen.sl.runtime.RequestContext;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Bean;

/** The Spring application of the trailing-slash test: the generated adapter around the slash contract. */
@SpringBootApplication(scanBasePackages = {"veldtest", "gen.sl.adapters"})
public class App {
  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  @Bean
  ItemsService itemsService() {
    return new ItemsService() {
      @Override
      public Item create(CreateRequest request, RequestContext ctx) {
        return request.body();
      }

      @Override
      public Integer count(CountRequest request, RequestContext ctx) {
        return 7;
      }
    };
  }

  @Bean
  ItemsHooks itemsHooks() {
    return new ItemsHooks();
  }
}
