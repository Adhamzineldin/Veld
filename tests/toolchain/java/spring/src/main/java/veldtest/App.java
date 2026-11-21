package veldtest;

import gen.a.orders.server.OrdersHooks;
import gen.w.live.server.VaultHooks;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import java.io.IOException;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Bean;
import org.springframework.web.filter.OncePerRequestFilter;

/**
 * A Spring Boot application around the generated Spring adapter. The generated
 * {@code OrdersController} is found by component scan; the service and its hooks are plain beans.
 * The application is copied next to the generated sources by crates/emitters/java/tests/toolchain.rs.
 */
@SpringBootApplication(scanBasePackages = {"veldtest", "gen.a.adapters", "gen.w.adapters"})
public class App {
  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  /** The implementation is itself the {@code OrdersService} bean. */
  @Bean
  ServerImpl serverImpl() {
    return new ServerImpl();
  }

  /** The streaming contract: both services, and the bearer hook of the Vault. */
  @Bean
  SpringStreamImpl.Live liveService() {
    return new SpringStreamImpl.Live();
  }

  @Bean
  SpringStreamImpl.Vault vaultService() {
    return new SpringStreamImpl.Vault();
  }

  @Bean
  VaultHooks vaultHooks() {
    return new VaultHooks((token, ctx) -> "good".equals(token) ? java.util.Map.of("user", "u1") : null);
  }

  @Bean
  OrdersHooks ordersHooks(ServerImpl impl) {
    return impl.hooks();
  }

  /** Counts the requests that are not probes, like the counting wrapper of the JDK round trip. */
  @Bean
  OncePerRequestFilter countRequests(ServerImpl impl) {
    return new OncePerRequestFilter() {
      @Override
      protected void doFilterInternal(
          HttpServletRequest request, HttpServletResponse response, FilterChain chain)
          throws ServletException, IOException {
        if (!request.getRequestURI().startsWith("/__probe")) {
          impl.requests.incrementAndGet();
        }
        chain.doFilter(request, response);
      }
    };
  }
}
