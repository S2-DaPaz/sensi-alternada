--[[
  Sensibilidade alternada para mouse Logitech G, no editor de script do G HUB.

  Por que isto existe: o painel deste repositorio escala o CURSOR do Windows, e o modo de
  tiro do BlueStacks nao le o cursor - le as contagens cruas do dispositivo. Trocar a DPI
  no firmware muda essas contagens, entao o jogo obedece. Verificado no aparelho do dono:
  mudar a DPI pelo software do mouse muda a sensibilidade no jogo; escalar o cursor nao.

  Onde colar: G HUB -> perfil do jogo -> menu do perfil -> SCRIPTING -> Script -> colar aqui
  -> Save. Amarre o perfil a C:\Program Files\BlueStacks_msi5\HD-Player.exe para o
  comportamento existir so dentro do emulador.
--]]

-- ---------------------------------------------------------------- ajuste aqui
local DPI_NORMAL  = 800   -- a DPI que voce usa para se movimentar
local DPI_TIRO    = 400   -- a DPI enquanto o gatilho esta pressionado
local BOTAO_TIRO  = 1     -- 1 esquerdo · 2 meio · 3 direito · 4 lateral · 5 lateral
-- -----------------------------------------------------------------------------

-- O botao esquerdo NAO chega ao script por padrao. Sem esta linha o OnEvent nunca ve o
-- botao 1 e o script parece morto, sem erro nenhum. O nome da funcao mudou entre versoes
-- do G HUB, entao tentamos as duas formas.
local function ligarBotaoPrimario()
  if not pcall(EnablePrimaryMouseButtonEvents, true) then
    pcall(EnablePrimaryMouseButtonEvent, true)
  end
end

function OnEvent(event, arg)
  if event == "PROFILE_ACTIVATED" then
    ligarBotaoPrimario()
    -- Indice 1 = normal, indice 2 = atirando.
    SetMouseDPITable({DPI_NORMAL, DPI_TIRO}, 1)

  elseif event == "MOUSE_BUTTON_PRESSED" and arg == BOTAO_TIRO then
    SetMouseDPITableIndex(2)

  elseif event == "MOUSE_BUTTON_RELEASED" and arg == BOTAO_TIRO then
    SetMouseDPITableIndex(1)

  elseif event == "PROFILE_DEACTIVATED" then
    -- Sair do jogo com o gatilho pressionado nao pode deixar a DPI baixa no desktop.
    SetMouseDPITableIndex(1)
  end
end
