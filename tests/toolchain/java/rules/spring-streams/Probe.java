package veldtest;

import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

/** Lists what the rule hook has seen, one line per violation, so a test can read it. */
@RestController
public class Probe {
  @GetMapping("/__events")
  public String events() {
    return String.join("\n", App.EVENTS);
  }
}
