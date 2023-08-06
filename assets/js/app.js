const appDiv = document.getElementById("app");
const wsEndpoint = appDiv.dataset.wsEndpoint;
const roomId = appDiv.dataset.roomId;
const currentUserId = appDiv.dataset.userId;
const ws = new WebSocket(`${wsEndpoint}/${roomId}`);

ws.onopen = sendGetCurrentState;

ws.onmessage = function(evt) {
  const roomMessage = JSON.parse(evt.data);
  console.log(roomMessage);

  if (roomMessage.msgType instanceof String) {
    console.log("roomMessage is a string type ", roomMessage.msgType);
  } else {
    if (roomMessage.msgType.waitingForPlayers) {
      // update screen with waiting for players

      let divWaitingForPlayers = appDiv.querySelector("#waitingForPlayers");

      if (divWaitingForPlayers) {
        divWaitingForPlayers.innerHTML = "";
      } else {
        divWaitingForPlayers = document.createElement("div");
        divWaitingForPlayers.id = "waitingForPlayers";
        appDiv.appendChild(divWaitingForPlayers);
      }

      let playerAlreadyJoined = roomMessage.msgType.waitingForPlayers.includes(currentUserId);

      for (const player of roomMessage.msgType.waitingForPlayers) {
        if (player) {
          let p = document.createElement("p");
          p.textContent = `${currentUserId === player ? "You" : "Player " + player} joined`;
          divWaitingForPlayers.appendChild(p);
        } else {
          let a = document.createElement("a");
          a.href = "#join";
          a.textContent = "Join";
          a.className = "d-block";
          if (!playerAlreadyJoined) {
            a.onclick = sendJoin;
          }
          divWaitingForPlayers.appendChild(a);
        }
      }
    }

  }
};

ws.onclose = function() {
  // websocket is closed.
  console.log("Connection is closed...");
};




// helper send msg
function sendGetCurrentState() {
  sendStringMessageType("getCurrentState");
}
function sendJoin() {
  sendStringMessageType("join");
}
function sendStringMessageType(msgType) {
  ws.send(JSON.stringify({
    msgType: msgType
  }));
}
