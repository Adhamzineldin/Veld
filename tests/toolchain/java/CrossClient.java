package veldtest;

/** Runs the client scenario against a server in another process: {@code CrossClient <base url> <label>}. */
public final class CrossClient {
  private CrossClient() {}

  public static void main(String[] args) throws Exception {
    Scenario.run(args[1], args[0], null);
    System.out.println("cross client ok (" + args[1] + ")");
  }
}
