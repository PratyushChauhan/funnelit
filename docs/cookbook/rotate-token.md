# Rotate the endpoint token

1. Open **Configure**

![Configure tab showing the bearer token row with Rotate](/images/configure.png)

2. In the bearer token row, click **Rotate**
3. Copy the new token
4. Update every MCP host config that still has the old `Authorization: Bearer …` value

Old tokens stop working immediately. The new token is stored in the OS keychain (`funnelit` / `endpoint-token`).
