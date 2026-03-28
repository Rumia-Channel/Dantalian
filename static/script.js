document.getElementById("hello-btn").addEventListener("click", async () => {
    const res = await fetch("/api/hello");
    const data = await res.json();
    document.getElementById("hello-result").textContent = data.message;
});

document.getElementById("echo-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const input = document.getElementById("echo-input");
    const res = await fetch("/api/echo", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: input.value }),
    });
    const data = await res.json();
    document.getElementById("echo-result").textContent = data.you_said;
    input.value = "";
});
