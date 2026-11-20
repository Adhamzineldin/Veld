# The Java service: no dependencies besides the JDK, so Maven only compiles and packs it.
FROM maven:3.9-eclipse-temurin-17 AS build
WORKDIR /app
COPY pom.xml ./
COPY src ./src
RUN mvn -q -B package -DskipTests

FROM eclipse-temurin:17-jre
WORKDIR /app
COPY --from=build /app/target/transaction-service.jar ./transaction-service.jar
ENV HOST=0.0.0.0
USER nobody
CMD ["java", "-jar", "transaction-service.jar"]
