namespace WizUI
{
    partial class MainScreen
    {
        /// <summary>
        /// Required designer variable.
        /// </summary>
        private System.ComponentModel.IContainer components = null;

        /// <summary>
        /// Clean up any resources being used.
        /// </summary>
        /// <param name="disposing">true if managed resources should be disposed; otherwise, false.</param>
        protected override void Dispose(bool disposing)
        {
            if (disposing && (components != null))
            {
                components.Dispose();
            }
            base.Dispose(disposing);
        }

        #region Windows Form Designer generated code

        /// <summary>
        /// Required method for Designer support - do not modify
        /// the contents of this method with the code editor.
        /// </summary>
        private void InitializeComponent()
        {
            this.lbProfiles = new System.Windows.Forms.ListBox();
            this.btnStartSnap = new System.Windows.Forms.Button();
            this.btnStartPlayback = new System.Windows.Forms.Button();
            this.btnNewProfile = new System.Windows.Forms.Button();
            this.tabControl1 = new System.Windows.Forms.TabControl();
            this.tabSettings = new System.Windows.Forms.TabPage();
            this.probTBModsPath = new System.Windows.Forms.TextBox();
            this.label2 = new System.Windows.Forms.Label();
            this.profBtnExeBrowse = new System.Windows.Forms.Button();
            this.profTBExePath = new System.Windows.Forms.TextBox();
            this.label1 = new System.Windows.Forms.Label();
            this.tabMods = new System.Windows.Forms.TabPage();
            this.tabLogs = new System.Windows.Forms.TabPage();
            this.btnDeleteProfile = new System.Windows.Forms.Button();
            this.btnCreateMod = new System.Windows.Forms.Button();
            this.tabControl1.SuspendLayout();
            this.tabSettings.SuspendLayout();
            this.SuspendLayout();
            // 
            // lbProfiles
            // 
            this.lbProfiles.FormattingEnabled = true;
            this.lbProfiles.Location = new System.Drawing.Point(12, 41);
            this.lbProfiles.Name = "lbProfiles";
            this.lbProfiles.Size = new System.Drawing.Size(120, 186);
            this.lbProfiles.TabIndex = 0;
            // 
            // btnStartSnap
            // 
            this.btnStartSnap.Location = new System.Drawing.Point(138, 12);
            this.btnStartSnap.Name = "btnStartSnap";
            this.btnStartSnap.Size = new System.Drawing.Size(120, 23);
            this.btnStartSnap.TabIndex = 1;
            this.btnStartSnap.Text = "Start (Snapshot)";
            this.btnStartSnap.UseVisualStyleBackColor = true;
            this.btnStartSnap.Click += new System.EventHandler(this.button1_Click);
            // 
            // btnStartPlayback
            // 
            this.btnStartPlayback.Location = new System.Drawing.Point(12, 12);
            this.btnStartPlayback.Name = "btnStartPlayback";
            this.btnStartPlayback.Size = new System.Drawing.Size(120, 23);
            this.btnStartPlayback.TabIndex = 2;
            this.btnStartPlayback.Text = "Start (Playback)";
            this.btnStartPlayback.UseVisualStyleBackColor = true;
            // 
            // btnNewProfile
            // 
            this.btnNewProfile.Location = new System.Drawing.Point(12, 233);
            this.btnNewProfile.Name = "btnNewProfile";
            this.btnNewProfile.Size = new System.Drawing.Size(120, 23);
            this.btnNewProfile.TabIndex = 3;
            this.btnNewProfile.Text = "New Profile";
            this.btnNewProfile.UseVisualStyleBackColor = true;
            // 
            // tabControl1
            // 
            this.tabControl1.Controls.Add(this.tabSettings);
            this.tabControl1.Controls.Add(this.tabMods);
            this.tabControl1.Controls.Add(this.tabLogs);
            this.tabControl1.Location = new System.Drawing.Point(138, 41);
            this.tabControl1.Name = "tabControl1";
            this.tabControl1.SelectedIndex = 0;
            this.tabControl1.Size = new System.Drawing.Size(628, 374);
            this.tabControl1.TabIndex = 4;
            // 
            // tabSettings
            // 
            this.tabSettings.Controls.Add(this.probTBModsPath);
            this.tabSettings.Controls.Add(this.label2);
            this.tabSettings.Controls.Add(this.profBtnExeBrowse);
            this.tabSettings.Controls.Add(this.profTBExePath);
            this.tabSettings.Controls.Add(this.label1);
            this.tabSettings.Location = new System.Drawing.Point(4, 22);
            this.tabSettings.Name = "tabSettings";
            this.tabSettings.Padding = new System.Windows.Forms.Padding(3);
            this.tabSettings.Size = new System.Drawing.Size(620, 348);
            this.tabSettings.TabIndex = 0;
            this.tabSettings.Text = "Settings";
            this.tabSettings.UseVisualStyleBackColor = true;
            // 
            // probTBModsPath
            // 
            this.probTBModsPath.Location = new System.Drawing.Point(10, 67);
            this.probTBModsPath.Name = "probTBModsPath";
            this.probTBModsPath.Size = new System.Drawing.Size(461, 20);
            this.probTBModsPath.TabIndex = 4;
            // 
            // label2
            // 
            this.label2.AutoSize = true;
            this.label2.Location = new System.Drawing.Point(10, 50);
            this.label2.Name = "label2";
            this.label2.Size = new System.Drawing.Size(61, 13);
            this.label2.TabIndex = 3;
            this.label2.Text = "Mods Path:";
            // 
            // profBtnExeBrowse
            // 
            this.profBtnExeBrowse.Location = new System.Drawing.Point(477, 20);
            this.profBtnExeBrowse.Name = "profBtnExeBrowse";
            this.profBtnExeBrowse.Size = new System.Drawing.Size(120, 23);
            this.profBtnExeBrowse.TabIndex = 2;
            this.profBtnExeBrowse.Text = "Browse...";
            this.profBtnExeBrowse.UseVisualStyleBackColor = true;
            // 
            // profTBExePath
            // 
            this.profTBExePath.Location = new System.Drawing.Point(10, 23);
            this.profTBExePath.Name = "profTBExePath";
            this.profTBExePath.Size = new System.Drawing.Size(461, 20);
            this.profTBExePath.TabIndex = 1;
            // 
            // label1
            // 
            this.label1.AutoSize = true;
            this.label1.Location = new System.Drawing.Point(7, 7);
            this.label1.Name = "label1";
            this.label1.Size = new System.Drawing.Size(88, 13);
            this.label1.TabIndex = 0;
            this.label1.Text = "Executable Path:";
            // 
            // tabMods
            // 
            this.tabMods.Location = new System.Drawing.Point(4, 22);
            this.tabMods.Name = "tabMods";
            this.tabMods.Padding = new System.Windows.Forms.Padding(3);
            this.tabMods.Size = new System.Drawing.Size(620, 348);
            this.tabMods.TabIndex = 1;
            this.tabMods.Text = "Mods";
            this.tabMods.UseVisualStyleBackColor = true;
            // 
            // tabLogs
            // 
            this.tabLogs.Location = new System.Drawing.Point(4, 22);
            this.tabLogs.Name = "tabLogs";
            this.tabLogs.Padding = new System.Windows.Forms.Padding(3);
            this.tabLogs.Size = new System.Drawing.Size(620, 348);
            this.tabLogs.TabIndex = 2;
            this.tabLogs.Text = "Logs";
            this.tabLogs.UseVisualStyleBackColor = true;
            this.tabLogs.Click += new System.EventHandler(this.tabPage1_Click);
            // 
            // btnDeleteProfile
            // 
            this.btnDeleteProfile.Location = new System.Drawing.Point(12, 262);
            this.btnDeleteProfile.Name = "btnDeleteProfile";
            this.btnDeleteProfile.Size = new System.Drawing.Size(120, 23);
            this.btnDeleteProfile.TabIndex = 5;
            this.btnDeleteProfile.Text = "Delete Profile";
            this.btnDeleteProfile.UseVisualStyleBackColor = true;
            // 
            // btnCreateMod
            // 
            this.btnCreateMod.Location = new System.Drawing.Point(265, 11);
            this.btnCreateMod.Name = "btnCreateMod";
            this.btnCreateMod.Size = new System.Drawing.Size(120, 23);
            this.btnCreateMod.TabIndex = 6;
            this.btnCreateMod.Text = "Create Mod...";
            this.btnCreateMod.UseVisualStyleBackColor = true;
            // 
            // MainScreen
            // 
            this.AutoScaleDimensions = new System.Drawing.SizeF(6F, 13F);
            this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font;
            this.ClientSize = new System.Drawing.Size(778, 427);
            this.Controls.Add(this.btnCreateMod);
            this.Controls.Add(this.btnDeleteProfile);
            this.Controls.Add(this.tabControl1);
            this.Controls.Add(this.btnNewProfile);
            this.Controls.Add(this.btnStartPlayback);
            this.Controls.Add(this.btnStartSnap);
            this.Controls.Add(this.lbProfiles);
            this.Name = "MainScreen";
            this.Text = "MainScreen";
            this.tabControl1.ResumeLayout(false);
            this.tabSettings.ResumeLayout(false);
            this.tabSettings.PerformLayout();
            this.ResumeLayout(false);

        }

        #endregion

        public System.Windows.Forms.ListBox lbProfiles;
        public System.Windows.Forms.Button btnStartSnap;
        public System.Windows.Forms.Button btnStartPlayback;
        public System.Windows.Forms.Button btnNewProfile;
        private System.Windows.Forms.TabControl tabControl1;
        private System.Windows.Forms.TabPage tabSettings;
        private System.Windows.Forms.TabPage tabMods;
        private System.Windows.Forms.TabPage tabLogs;
        public System.Windows.Forms.TextBox probTBModsPath;
        private System.Windows.Forms.Label label2;
        public System.Windows.Forms.Button profBtnExeBrowse;
        public System.Windows.Forms.TextBox profTBExePath;
        private System.Windows.Forms.Label label1;
        public System.Windows.Forms.Button btnDeleteProfile;
        public System.Windows.Forms.Button btnCreateMod;
    }
}